use std::fmt;
use std::marker::PhantomData;

use spire_agent::objective::ObjectiveRegistry;
use spire_agent::schemars::JsonSchema;
use spire_agent::serde::de::DeserializeOwned;

use super::Handler;
use crate::backend::Session;
use crate::extract::{AgentExtract, FromContext};
use crate::worker::{IntoOutcome, Outcome};
use crate::{Context, TRACING_TARGET};

/// Collects model-extracted records into a dataset.
///
/// Extracts `Vec<T>` and stores each item. Empty answers write nothing.
/// Objective middleware automatically accepts successfully stored records
/// for a matching `Objective<T>`; ordinary collection needs no objective.
///
/// Register an [`Agent`](spire_agent::Agent) with [`with_agent`](spire_agent::AgentCrawlerExt::with_agent)
/// and the dataset on the crawler. Extraction, budget and dataset
/// errors propagate as they do in a handler using [`AgentExtract`] and
/// [`Data`](crate::extract::Data).
///
/// ```
/// use spire::agent::{Agent, AgentCrawlerExt};
/// use spire::backend::Noop;
/// use spire::dataset::InMemDataset;
/// use spire::handler::AgentCollect;
/// use spire::{Crawler, Router};
///
/// # fn crawler(agent: Agent) {
/// let router = Router::new().route("names", AgentCollect::<String>::new());
/// let crawler = Crawler::new(Noop, router)
///     .with_agent(agent)
///     .with_dataset(InMemDataset::<String>::new());
/// # let _ = crawler;
/// # }
/// ```
#[must_use]
pub struct AgentCollect<T> {
    marker: PhantomData<fn() -> T>,
}

impl<T> AgentCollect<T> {
    /// Collects every model-extracted `T` of the page.
    pub const fn new() -> Self {
        Self {
            marker: PhantomData,
        }
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
        }
    }
}

impl<T> fmt::Debug for AgentCollect<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentCollect")
            .field("item", &std::any::type_name::<T>())
            .finish()
    }
}

impl<C, S, T> Handler<C, AgentCollect<T>, S> for AgentCollect<T>
where
    C: Session,
    S: Send + Sync + 'static,
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    async fn call(&self, cx: &Context<C>, state: &S) -> Outcome {
        let objectives = cx.request().extensions().get::<ObjectiveRegistry>();
        if objectives.is_some_and(|registry| registry.is_finished() || registry.completed::<T>()) {
            return Outcome::Continue;
        }
        let data = match cx.dataset::<T>() {
            Ok(data) => data,
            Err(error) => return error.into_outcome(),
        };
        let AgentExtract(items) = match AgentExtract::<Vec<T>>::from_context(cx, state).await {
            Ok(items) => items,
            Err(outcome) => return outcome,
        };
        let evidence = match objectives
            .map(|registry| registry.prepare(&items, cx.request().uri()))
            .transpose()
        {
            Ok(evidence) => evidence.flatten(),
            Err(error) => return error.into_outcome(),
        };
        let collected = items.len();
        if let Err(error) = data.write_all(items).await {
            return error.into_outcome();
        }
        if let Some(evidence) = evidence {
            evidence.accept();
        }
        tracing::debug!(target: TRACING_TARGET, collected, "collected model-extracted items");
        Outcome::Continue
    }
}
