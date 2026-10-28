use spire_core::worker::{IntoOutcome, Outcome};

/// A model operation's result.
pub type Result<T> = std::result::Result<T, Error>;

/// Failures of provider calls, structured extraction, budgets and caching.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The provider failed.
    ///
    /// The provider owns its retries and rate limits.
    #[error("model provider failed: {0}")]
    Provider(#[source] spire_core::BoxError),
    /// A budget has run out; no further provider calls are admitted.
    #[error("model budget exhausted: {0}")]
    Budget(&'static str),
    /// Configuration or accounting cannot enforce a requested budget.
    #[error("invalid model configuration or usage: {0}")]
    Configuration(String),
    /// The answer was not valid JSON matching the requested schema/type.
    #[error("invalid model answer: {0}")]
    Answer(String),
    /// A cache operation failed.
    #[error("model cache failed: {0}")]
    Cache(String),
}

impl Error {
    /// Wraps a provider's error without asking the crawler to retry it.
    pub fn provider(error: impl Into<spire_core::BoxError>) -> Self {
        Self::Provider(error.into())
    }
}

impl IntoOutcome for Error {
    fn into_outcome(self) -> Outcome {
        let fatal = matches!(self, Self::Budget(_) | Self::Configuration(_));
        let error = self.into();
        if fatal {
            Outcome::Fatal(error)
        } else {
            Outcome::Fail(error)
        }
    }
}

impl From<Error> for spire_core::Error {
    /// Preserves the agent classification as the source of a crawler error.
    ///
    /// A converted error follows the ordinary crawler error outcome. Return
    /// the agent error directly to retain its fatal budget/configuration policy.
    fn from(error: Error) -> Self {
        let kind = if matches!(error, Error::Configuration(_)) {
            spire_core::ErrorKind::Invalid
        } else {
            spire_core::ErrorKind::Worker
        };
        Self::new(kind)
            .with_message(error.to_string())
            .with_source(error)
    }
}
