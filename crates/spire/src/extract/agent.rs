use serde::de::DeserializeOwned;
use spire_agent::Agent;
use spire_agent::schemars::JsonSchema;

use crate::Context;
use crate::backend::Session;
use crate::extract::html::Markdown;
use crate::extract::{Extension, FromContext};
use crate::worker::{IntoOutcome, Outcome};

/// Selects typed records from page text through the configured model agent.
///
/// The page is converted to Markdown before extraction. Register the agent
/// with [`with_agent`](spire_agent::AgentCrawlerExt::with_agent). Use `Vec<T>`
/// for zero or more records, or `Option<T>` when a record may be absent.
///
/// ```
/// use spire::agent::schemars::JsonSchema;
/// use spire::agent::serde::Deserialize;
/// use spire::extract::AgentExtract;
///
/// #[derive(Deserialize, JsonSchema)]
/// #[serde(crate = "spire::agent::serde")]
/// #[schemars(crate = "spire::agent::schemars")]
/// struct Product { name: String }
///
/// async fn products(AgentExtract(products): AgentExtract<Vec<Product>>) {
///     for product in products { println!("{}", product.name); }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentExtract<T>(pub T);

impl<C, S, T> FromContext<C, S> for AgentExtract<T>
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
