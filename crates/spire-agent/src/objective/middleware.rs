use std::any::TypeId;

use schemars::JsonSchema;
use serde::Serialize;
use spire_core::Context;
use spire_core::worker::{IntoOutcome, Outcome, Worker};

use super::{Objective, ObjectiveRegistry, ObjectiveStatus, StopPolicy};
use crate::{Agent, Error};

/// Adds objective middleware to a worker, such as Spire's router.
///
/// Configure routes first. Further `with_objective` calls add to the same
/// wrapper, sharing one registry and one stopping decision.
pub trait ObjectiveExt: Sized {
    /// Wraps this worker and registers its first typed objective.
    fn with_objective<C, T>(self, objective: Objective<T>) -> ObjectiveWorker<Self>
    where
        C: Send + Sync + 'static,
        Self: Worker<C>,
        T: Serialize + JsonSchema + 'static,
    {
        ObjectiveWorker {
            inner: self,
            registry: ObjectiveRegistry::default(),
        }
        .with_objective(objective)
    }
}
impl<W> ObjectiveExt for W {}

/// A worker with automatically supplied objective services and shared progress.
///
/// Clones share objective progress. Work exhaustion leaves unmet goals intact;
/// adding seeds and running again continues the same objectives. Use a new
/// wrapper for fresh progress. No crawler or frontier changes are required.
#[derive(Clone)]
pub struct ObjectiveWorker<W> {
    inner: W,
    registry: ObjectiveRegistry,
}

impl<W> ObjectiveWorker<W> {
    /// Adds another independently tracked record type to this middleware.
    #[must_use]
    pub fn with_objective<T: Serialize + JsonSchema + 'static>(
        mut self,
        objective: Objective<T>,
    ) -> Self {
        self.registry.add(objective);
        self
    }

    /// Stops after all objectives finish, or after any objective finishes.
    #[must_use]
    pub fn stop_when(mut self, stop: StopPolicy) -> Self {
        self.registry.set_stop_policy(stop);
        self
    }

    /// Returns shared progress without exposing a manually managed runtime.
    ///
    /// # Errors
    /// Returns invalid configuration for an unregistered record type.
    pub fn objective<T: 'static>(&self) -> spire_core::Result<ObjectiveStatus<T>> {
        self.registry.get::<T>()
    }
}

impl<C, W> Worker<C> for ObjectiveWorker<W>
where
    C: Send + Sync + 'static,
    W: Worker<C>,
{
    async fn handle(&self, mut cx: Context<C>) -> Outcome {
        if let Err(error) = self.registry.validate() {
            return error.into_outcome();
        }
        if cx
            .request()
            .extensions()
            .get::<ObjectiveRegistry>()
            .is_some()
        {
            return Error::Configuration(
                "nested objective middleware: register objectives on the same wrapper".into(),
            )
            .into_outcome();
        }
        let Some(agent) = cx.request().extensions().get::<Agent>().cloned() else {
            return Error::Configuration("objective middleware requires .with_agent(agent)".into())
                .into_outcome();
        };
        if self.registry.is_finished() {
            return Outcome::Stop;
        }
        let source = cx.request().uri().to_string();
        let active = self
            .registry
            .entries()
            .iter()
            .filter(|entry| entry.active())
            .map(|entry| entry.id)
            .collect::<Vec<TypeId>>();
        cx.request_mut()
            .extensions_mut()
            .insert(self.registry.clone());
        let outcome = self.inner.handle(cx).await;
        if !matches!(outcome, Outcome::Continue) {
            return outcome;
        }
        if let Err(error) = self.registry.page_done(&source, &active).await {
            return error.into_outcome();
        }
        if let Err(error) = self.registry.check(&agent).await {
            return error.into_outcome();
        }
        if self.registry.is_finished() {
            Outcome::Stop
        } else {
            outcome
        }
    }
}
