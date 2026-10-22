use std::convert::Infallible;
use std::time::Duration;

use crate::request::Tag;
use crate::{BoxError, Error};

/// What the crawler should do after a worker handled a request.
#[must_use]
#[derive(Debug, Default)]
pub enum Outcome {
    /// The request is done.
    #[default]
    Continue,
    /// Try the request again, after the given delay or the frontier's
    /// default delay.
    Retry {
        /// Delay before the next attempt; `None` defers to the frontier.
        after: Option<Duration>,
    },
    /// Pause every request in `scope` for `duration`, then try this request
    /// again. Typically returned on `429 Too Many Requests`.
    Backoff {
        /// The requests to pause.
        scope: Scope,
        /// How long to pause them.
        duration: Duration,
    },
    /// Stop crawling `scope`: its queued requests are skipped from now on.
    /// Returned on the first request of a tag, this drops the whole tag.
    Abort(Scope),
    /// Stop the crawl: requests in flight finish, nothing new starts.
    Stop,
    /// The request failed. Retryable errors are tried again, others are
    /// recorded as failures.
    Fail(Error),
}

impl Outcome {
    /// Retries the request after the frontier's default delay.
    pub const fn retry() -> Self {
        Self::Retry { after: None }
    }

    /// Retries the request after `delay`.
    pub const fn retry_after(delay: Duration) -> Self {
        Self::Retry { after: Some(delay) }
    }

    /// Pauses requests with the same tag as this one for `duration`.
    pub const fn backoff(duration: Duration) -> Self {
        Self::Backoff {
            scope: Scope::SameTag,
            duration,
        }
    }

    /// Skips every further request with the same tag as this one.
    pub const fn abort() -> Self {
        Self::Abort(Scope::SameTag)
    }

    /// Skips every further request with `tag`.
    pub fn abort_tag(tag: impl Into<Tag>) -> Self {
        Self::Abort(Scope::Tag(tag.into()))
    }

    /// Pauses requests with `tag` for `duration`.
    pub fn backoff_tag(tag: impl Into<Tag>, duration: Duration) -> Self {
        Self::Backoff {
            scope: Scope::Tag(tag.into()),
            duration,
        }
    }
}

/// The requests an [`Outcome::Backoff`] or [`Outcome::Abort`] applies to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Scope {
    /// Requests sharing the tag of the request that was handled, including
    /// untagged requests when it had none.
    #[default]
    SameTag,
    /// Requests with the given tag.
    Tag(Tag),
}

/// Conversion into an [`Outcome`], implemented for handler return types.
pub trait IntoOutcome {
    /// Converts `self` into an [`Outcome`].
    fn into_outcome(self) -> Outcome;
}

impl IntoOutcome for Outcome {
    fn into_outcome(self) -> Outcome {
        self
    }
}

impl IntoOutcome for () {
    fn into_outcome(self) -> Outcome {
        Outcome::Continue
    }
}

impl IntoOutcome for Infallible {
    fn into_outcome(self) -> Outcome {
        match self {}
    }
}

impl IntoOutcome for Error {
    fn into_outcome(self) -> Outcome {
        Outcome::Fail(self)
    }
}

impl IntoOutcome for BoxError {
    /// Fails with the error, keeping its kind if it is an [`Error`] and
    /// classifying it as a [`Worker`](crate::ErrorKind::Worker) error
    /// otherwise.
    fn into_outcome(self) -> Outcome {
        Outcome::Fail(Error::from_boxed(crate::ErrorKind::Worker, self))
    }
}

impl<T> IntoOutcome for Option<T>
where
    T: IntoOutcome,
{
    fn into_outcome(self) -> Outcome {
        self.map_or(Outcome::Continue, IntoOutcome::into_outcome)
    }
}

impl<T, E> IntoOutcome for Result<T, E>
where
    T: IntoOutcome,
    E: IntoOutcome,
{
    fn into_outcome(self) -> Outcome {
        match self {
            Ok(value) => value.into_outcome(),
            Err(error) => error.into_outcome(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorKind;

    #[test]
    fn conversions() {
        assert!(matches!(().into_outcome(), Outcome::Continue));
        assert!(matches!(None::<Outcome>.into_outcome(), Outcome::Continue));

        let ok: Result<(), Error> = Ok(());
        assert!(matches!(ok.into_outcome(), Outcome::Continue));

        let err: Result<(), Error> = Err(Error::new(ErrorKind::Http));
        assert!(matches!(err.into_outcome(), Outcome::Fail(_)));

        let retry: Result<(), Outcome> = Err(Outcome::retry());
        assert!(matches!(
            retry.into_outcome(),
            Outcome::Retry { after: None }
        ));
    }
}
