//! Signal-based flow control for web scraping tasks.
//!
//! This module provides the [`Signal`] type for controlling task execution flow,
//! including success/failure handling, waiting, and aborting tasks based on tags.

use std::convert::Infallible;
use std::time::Duration;

use crate::context::Tag;
use crate::{BoxError, Error};

/// Defines a way to select or filter [`Tag`]s for signal operations.
#[derive(Debug, Default, Clone)]
pub enum TagQuery {
    /// Matches the same [`Tag`] as used by the [`Request`].
    ///
    /// Does not match [`Tag::Fallback`].
    ///
    /// [`Request`]: crate::context::Request
    #[default]
    Owner,

    /// Matches the provided [`Tag`].
    Single(Tag),

    /// Matches every [`Tag`] from the provided list.
    List(Vec<Tag>),

    /// Matches every [`Tag`], including [`Tag::Fallback`].
    Every,
}

impl TagQuery {
    /// Matches a [`Tag`] to the owned [`TagQuery`].
    pub(crate) fn is_match(&self, tag: &Tag, owner: &Tag) -> bool {
        match self {
            Self::Owner => !owner.is_fallback() && tag == owner,
            Self::Single(x) => x == tag,
            Self::List(x) => x.contains(tag),
            Self::Every => true,
        }
    }
}

/// Represents various events that can be emitted during [`Request`] processing.
///
/// Signals are used to tell whether it should exit early or go on as usual,
/// similar to the standard library's [`ControlFlow`] enum.
///
/// [`Request`]: crate::context::Request
/// [`ControlFlow`]: std::ops::ControlFlow
#[must_use]
#[derive(Debug, Default)]
pub enum Signal {
    /// Task succeeded, immediately proceed with another task.
    #[default]
    Continue,
    /// Task failed, immediately proceed with another task.
    Skip,

    /// Task succeeded, wait before tasks with matching tags.
    Wait(TagQuery, Duration),
    /// Task failed, wait before tasks with matching tags.
    Hold(TagQuery, Duration),

    /// Task failed, terminate all collector tasks.
    Fail(TagQuery, BoxError),
}

impl Signal {
    /// Creates a new [`Signal`] from the boxable error.
    pub fn error(error: impl Into<BoxError>) -> Self {
        error.into().into_signal()
    }

    /// Returns the [`Duration`] if applicable, default otherwise.
    #[must_use]
    pub fn duration(&self) -> Duration {
        match self {
            Self::Wait(_, x) | Self::Hold(_, x) => *x,
            _ => Duration::default(),
        }
    }

    /// Returns the [`TagQuery`] if applicable, default otherwise.
    #[must_use]
    pub fn query(&self) -> TagQuery {
        match self {
            Self::Wait(x, _) | Self::Hold(x, _) | Self::Fail(x, _) => x.clone(),
            _ => TagQuery::default(),
        }
    }
}

/// Trait for generating [`Signal`]s.
pub trait IntoSignal {
    /// Transforms `self` into the [`Signal`].
    fn into_signal(self) -> Signal;
}

impl IntoSignal for Signal {
    #[inline]
    fn into_signal(self) -> Signal {
        self
    }
}

impl IntoSignal for () {
    #[inline]
    fn into_signal(self) -> Signal {
        Signal::Continue
    }
}

impl IntoSignal for Infallible {
    #[inline]
    fn into_signal(self) -> Signal {
        Signal::Continue
    }
}

impl IntoSignal for Duration {
    #[inline]
    fn into_signal(self) -> Signal {
        Signal::Wait(TagQuery::default(), self)
    }
}

impl IntoSignal for BoxError {
    #[inline]
    fn into_signal(self) -> Signal {
        Error::from_boxed(self).into_signal()
    }
}

impl<T> IntoSignal for Option<T>
where
    T: IntoSignal,
{
    fn into_signal(self) -> Signal {
        self.map_or_else(|| ().into_signal(), IntoSignal::into_signal)
    }
}

impl<T, E> IntoSignal for Result<T, E>
where
    T: IntoSignal,
    E: IntoSignal,
{
    fn into_signal(self) -> Signal {
        fn flip(x: Signal) -> Signal {
            match x {
                Signal::Continue => Signal::Skip,
                Signal::Skip => Signal::Continue,
                Signal::Wait(q, x) => Signal::Hold(q, x),
                Signal::Hold(q, x) => Signal::Wait(q, x),
                Signal::Fail(q, x) => Signal::Fail(q, x),
            }
        }

        match self {
            Ok(x) => x.into_signal(),
            Err(x) => flip(x.into_signal()),
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::context::{IntoSignal, Signal};

    #[test]
    fn basic() {
        assert!(matches!(().into_signal(), Signal::Continue));
        let data = Duration::default();
        assert!(matches!(data.into_signal(), Signal::Wait(..)));
    }

    #[test]
    fn with_option() {
        let flip: Option<Duration> = Some(Duration::default());
        assert!(matches!(flip.into_signal(), Signal::Wait(..)));
        let flip: Option<Duration> = None;
        assert!(matches!(flip.into_signal(), Signal::Continue));
    }

    #[test]
    fn with_result() {
        let flip: Result<Duration, Duration> = Ok(Duration::default());
        assert!(matches!(flip.into_signal(), Signal::Wait(..)));
        let flip: Result<Duration, Duration> = Err(Duration::default());
        assert!(matches!(flip.into_signal(), Signal::Hold(..)));
    }
}
