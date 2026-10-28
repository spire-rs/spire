use serde::de::DeserializeOwned;
use spire_agent::schemars::JsonSchema;
use spire_agent::{Agent, Extract};

use crate::Context;
use crate::backend::Session;
use crate::extract::html::Markdown;
use crate::extract::{Extension, FromContext};
use crate::worker::{IntoOutcome, Outcome};

impl<C, S, T> FromContext<C, S> for Extract<T>
where
    C: Session,
    S: Sync,
    T: DeserializeOwned + JsonSchema + Send,
{
    type Rejection = Outcome;

    async fn from_context(cx: &Context<C>, state: &S) -> Result<Self, Outcome> {
        let Extension(agent) = Extension::<Agent>::from_context(cx, state)
            .await
            .map_err(IntoOutcome::into_outcome)?;
        let Markdown(markdown) = Markdown::from_context(cx, state)
            .await
            .map_err(IntoOutcome::into_outcome)?;
        agent
            .extract(&markdown)
            .await
            .map(Self)
            .map_err(IntoOutcome::into_outcome)
    }
}
