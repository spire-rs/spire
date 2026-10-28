use std::future::Future;

use futures::future::BoxFuture;
use serde::Serialize;
use serde_json::Value;

use crate::{Result, Usage};

/// A model completion request.
///
/// The schema constrains the complete JSON answer.
#[derive(Debug, Clone, Serialize)]
pub struct CompletionRequest {
    /// System instructions, separate from untrusted input.
    pub system: String,
    /// The page or other text to extract from.
    pub input: String,
    /// An optional JSON Schema for structured output.
    pub schema: Option<Value>,
    /// The maximum number of output tokens.
    pub max_tokens: u64,
}

/// A model answer and its reported usage.
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    /// The answer text, without Markdown fences or tool-call wrappers.
    pub text: String,
    /// The reported usage, including cost when known.
    pub usage: Usage,
}

/// Makes model completion calls.
///
/// Implementations own retries and rate limits.
pub trait Provider: Send + Sync + 'static {
    /// Completes a request, reporting usage for the whole call.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider cannot produce an answer.
    fn complete(
        &self,
        request: CompletionRequest,
    ) -> impl Future<Output = Result<CompletionResponse>> + Send;
}

pub(crate) trait Service: Send + Sync {
    fn call(&self, request: CompletionRequest) -> BoxFuture<'_, Result<CompletionResponse>>;
}

impl<P: Provider> Service for P {
    fn call(&self, request: CompletionRequest) -> BoxFuture<'_, Result<CompletionResponse>> {
        Box::pin(self.complete(request))
    }
}
