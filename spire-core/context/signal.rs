use std::convert::Infallible;
use std::time::Duration;

use crate::context::Tag;
use crate::{BoxError, Error};

/// Defines a way to select or filter out [`Tag`]s.
#[derive(Debug, Default, Clone)]
pub enum TagQuery {
    /// Matches the same [`Tag`] as used by the [`Request`].
    ///
    /// Does not match [`Tag::Fallback`].
    ///
    /// [`Request`]: crate::context::Request
    #[default]
    Owner,

    /// Matches every [`Tag`], including [`Tag::Fallback`].
    Every,

    /// Matches every [`Tag`], but from the provided list.
    Exclude(Vec<Tag>),

    /// Matches every [`Tag`] from the provided list.
    Include(Vec<Tag>),
}

impl TagQuery {
    /// Matches a [`Tag`] to the owned [`TagQuery`].
    pub(crate) fn is_match(&self, tag: &Tag, owner: &Tag) -> bool {
        match self {
            TagQuery::Owner => !owner.is_fallback() && tag == owner,
            TagQuery::Every => true,
            TagQuery::Exclude(x) => !x.contains(tag),
            TagQuery::Include(x) => x.contains(tag),
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
#[derive(Debug, Default)]
pub enum Signal {
    /// Task succeeded, immediately proceed with another task.
    #[default]
    Continue,
    /// Task failed, immediately proceed with another task.
    Skip,

    /// Task succeeded, wait before tasks with matching tags.
    Wait(TagQuery, Duration),
    /// Task failed, wait before repeating this task.
    Repeat(TagQuery, Duration),

    /// Task failed, terminate all collector tasks.
    Stop(TagQuery, Error),
}

impl Signal {
    pub fn error(error: impl Into<BoxError>) -> Self {
        Signal::Stop(TagQuery::Owner, Error::new(error))
    }

    /// Returns the provided [`Duration`] if applicable, default otherwise.
    pub fn duration(&self) -> Duration {
        match self {
            Signal::Wait(_, x) => *x,
            Signal::Repeat(_, x) => *x,
            _ => Duration::default(),
        }
    }

    // Returns the provided [`Query`] if applicable, default otherwise.
    pub fn query(&self) -> TagQuery {
        match self {
            Signal::Wait(x, _) => x.clone(),
            Signal::Repeat(x, _) => x.clone(),
            Signal::Stop(x, _) => x.clone(),
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
    fn into_signal(self) -> Signal {
        self
    }
}

impl IntoSignal for () {
    fn into_signal(self) -> Signal {
        Signal::Continue
    }
}

impl IntoSignal for Infallible {
    fn into_signal(self) -> Signal {
        Signal::Continue
    }
}

impl IntoSignal for Duration {
    fn into_signal(self) -> Signal {
        Signal::Wait(TagQuery::default(), self)
    }
}

impl IntoSignal for Error {
    fn into_signal(self) -> Signal {
        Signal::Stop(TagQuery::Every, self)
    }
}

impl IntoSignal for BoxError {
    fn into_signal(self) -> Signal {
        Error::new(self).into_signal()
    }
}

impl<T> IntoSignal for Option<T>
where
    T: IntoSignal,
{
    fn into_signal(self) -> Signal {
        match self {
            Some(x) => x.into_signal(),
            None => Signal::Continue,
        }
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
                Signal::Wait(q, x) => Signal::Repeat(q, x),
                Signal::Repeat(q, x) => Signal::Wait(q, x),
                Signal::Stop(q, x) => Signal::Stop(q, x),
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
        assert!(matches!(flip.into_signal(), Signal::Repeat(..)));
    }
}
