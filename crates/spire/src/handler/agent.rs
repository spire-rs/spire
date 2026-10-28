use std::fmt;
use std::marker::PhantomData;

use spire_agent::objective::ObjectiveRun;
use spire_agent::schemars::JsonSchema;
use spire_agent::serde::Serialize;
use spire_agent::serde::de::DeserializeOwned;
use spire_agent::serde_json::Value;
use spire_agent::{Agent, Extract};

use super::Handler;
use crate::backend::Session;
use crate::extract::{FromContext, FromRef};
use crate::worker::{IntoOutcome, Outcome};
use crate::{Context, TRACING_TARGET};

type Encode<T> = fn(&[T]) -> crate::Result<Vec<Value>>;

/// A handler that extracts `Vec<T>` from a page with a model and stores the
/// items in the `T` dataset. Empty answers write nothing.
///
/// Put an [`Agent`] in router state, directly or through [`FromRef`], and
/// register the dataset on the crawler. Extraction, budget and dataset
/// errors propagate as they do in a handler using [`Extract`] and
/// [`Data`](crate::dataset::Data).
///
/// ```
/// use spire::agent::Agent;
/// use spire::handler::AgentCollect;
/// use spire::Router;
///
/// # fn router(agent: Agent) {
/// let router: Router<spire::backend::Noop, Agent> = Router::with_state(agent)
///     .route("names", AgentCollect::<String>::new());
/// # let _ = router;
/// # }
/// ```
#[must_use]
pub struct AgentCollect<T> {
    marker: PhantomData<fn() -> T>,
    report: Option<(ObjectiveRun, Encode<T>)>,
}

impl<T> AgentCollect<T> {
    /// Collects every model-extracted `T` of the page.
    pub const fn new() -> Self {
        Self {
            marker: PhantomData,
            report: None,
        }
    }

    /// Reports this batch to an objective only after all dataset writes
    /// succeed. Values are counted as supplied; uniqueness is application policy.
    /// Serializing evidence happens before writing, so serialization failure
    /// cannot leave stored items with an unreportable batch.
    pub fn report_to(mut self, objective: &ObjectiveRun) -> Self
    where
        T: Serialize,
    {
        self.report = Some((objective.clone(), |items| {
            items
                .iter()
                .map(spire_agent::serde_json::to_value)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| crate::Error::worker(error.to_string()).with_source(error))
        }));
        self
    }
}

impl<T> Default for AgentCollect<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for AgentCollect<T> {
    fn clone(&self) -> Self {
        Self {
            marker: PhantomData,
            report: self.report.clone(),
        }
    }
}

impl<T> fmt::Debug for AgentCollect<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentCollect")
            .field("item", &std::any::type_name::<T>())
            .field("report", &self.report.as_ref().map(|(run, _)| run))
            .finish()
    }
}

impl<C, S, T> Handler<C, AgentCollect<T>, S> for AgentCollect<T>
where
    C: Session,
    S: Send + Sync + 'static,
    Agent: FromRef<S>,
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    async fn call(&self, cx: &Context<C>, state: &S) -> Outcome {
        let data = match cx.dataset::<T>() {
            Ok(data) => data,
            Err(error) => return error.into_outcome(),
        };
        let Extract(items) = match Extract::<Vec<T>>::from_context(cx, state).await {
            Ok(items) => items,
            Err(outcome) => return outcome,
        };
        let evidence = match &self.report {
            Some((_, encode)) => match encode(&items) {
                Ok(evidence) => Some(evidence),
                Err(error) => return error.into_outcome(),
            },
            None => None,
        };
        let collected = items.len();
        if let Err(error) = data.write_all(items).await {
            return error.into_outcome();
        }
        if let (Some((run, _)), Some(evidence)) = (&self.report, evidence) {
            if let Err(error) = run.record_page(&cx.request().uri().to_string(), evidence) {
                return error.into_outcome();
            }
            if let Err(error) = run.check().await {
                return error.into_outcome();
            }
            if run.finished().is_some() {
                return Outcome::Stop;
            }
        }
        tracing::debug!(target: TRACING_TARGET, collected, "collected model-extracted items");
        Outcome::Continue
    }
}
