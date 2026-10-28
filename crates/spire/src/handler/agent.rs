use std::fmt;
use std::marker::PhantomData;

use spire_agent::schemars::JsonSchema;
use spire_agent::serde::de::DeserializeOwned;
use spire_agent::{Agent, Extract};

use super::Handler;
use crate::backend::Session;
use crate::extract::{FromContext, FromRef};
use crate::worker::{IntoOutcome, Outcome};
use crate::{Context, TRACING_TARGET};

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
        Self::new()
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
        let collected = items.len();
        if let Err(error) = data.write_all(items).await {
            return error.into_outcome();
        }
        tracing::debug!(target: TRACING_TARGET, collected, "collected model-extracted items");
        Outcome::Continue
    }
}
