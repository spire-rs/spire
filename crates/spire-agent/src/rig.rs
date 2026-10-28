//! Rig completion models as Spire providers.

pub use ::rig;
use rig::DynModel;
use rig::completion::{CompletionRequest as RigRequest, FinishReason};
use rig::message::AssistantContent;
use rig::operation::Completion as RigCompletion;

use crate::{CompletionRequest, CompletionResponse, Error, Provider, Result, Usage};

/// Wraps any Rig completion model. Schemas are included in system
/// instructions by default; opt into native schemas for models supporting
/// them. Responses are always validated by the agent before caching.
#[derive(Clone)]
pub struct Rig {
    model: DynModel<RigCompletion>,
    native_schema: bool,
}

impl Rig {
    /// Wraps a model without adding retries or a rate-limit policy.
    #[must_use]
    pub fn new(model: impl Into<DynModel<RigCompletion>>) -> Self {
        Self {
            model: model.into(),
            native_schema: false,
        }
    }

    /// Also forwards the schema as Rig's native structured-output schema.
    /// Enable this only when the selected provider/model supports it.
    #[must_use]
    pub const fn with_native_schema(mut self) -> Self {
        self.native_schema = true;
        self
    }
}

impl Provider for Rig {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let system = match &request.schema {
            Some(schema) => format!(
                "{}\n\nRespond with JSON matching this schema:\n{schema}",
                request.system
            ),
            None => request.system,
        };
        let mut call = RigRequest::new(request.input)
            .preamble(system)
            .max_tokens(request.max_tokens);
        if self.native_schema {
            call.output_schema = request
                .schema
                .map(TryInto::try_into)
                .transpose()
                .map_err(|error| Error::Configuration(format!("invalid native schema: {error}")))?;
        }
        let response = self.model.call(call).await.map_err(Error::provider)?;
        let known_bad = response.error.is_some()
            || response.aborted.is_some()
            || response
                .finish_reason()
                .is_some_and(|reason| !matches!(reason, FinishReason::Stop));
        // Preserve paid usage even on refusals/truncation: the agent accounts
        // it first, then rejects the empty answer without caching it.
        let text = if known_bad {
            String::new()
        } else {
            response
                .choice
                .iter()
                .filter_map(|content| match content {
                    AssistantContent::Text(text) => Some(text.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("")
        };
        let reported = response.usage;
        let cost = reported.cost;
        Ok(CompletionResponse {
            text,
            usage: Usage {
                input_tokens: reported.input_tokens,
                output_tokens: reported.output_tokens,
                cost_usd: cost.map(|cost| cost.total),
            },
        })
    }
}
