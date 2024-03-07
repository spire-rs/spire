use std::convert::Infallible;
use std::error::Error;
use std::time::Duration;

use crate::context::Tag;
use crate::dataset;

///
/// [`ControlFlow`]: std::ops::ControlFlow
#[derive(Debug, Default)]
pub enum Signal {
    /// Task processed, immediately proceed with another task.
    #[default]
    Continue,
    /// Task failed, immediately proceed with another task.
    Skip,

    /// Task processed, wait before tasks with matching tags.
    Wait(Tag, Duration),
    /// Task failed, wait before tasks with matching tags.
    Repeat(Tag, Duration),

    /// Task failed, terminate all collector tasks.
    Stop(Box<dyn Error + Send + Sync>),
}

impl Signal {
    pub fn duration(&self) -> Duration {
        match self {
            Signal::Wait(_, x) => *x,
            Signal::Repeat(_, x) => *x,
            _ => Duration::from_secs(0),
        }
    }
}

pub trait IntoSignal {
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
        Signal::Wait(Tag::default(), self)
    }
}

impl IntoSignal for dataset::Error {
    fn into_signal(self) -> Signal {
        todo!()
    }
}

impl IntoSignal for Box<dyn Error + Send + Sync> {
    fn into_signal(self) -> Signal {
        Signal::Stop(self)
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
                Signal::Wait(t, x) => Signal::Repeat(t, x),
                Signal::Repeat(t, x) => Signal::Wait(t, x),
                Signal::Stop(x) => Signal::Stop(x),
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
