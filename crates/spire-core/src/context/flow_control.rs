//! FlowControl-based flow control for web scraping tasks.
//!
//! This module provides the [`FlowControl`] type for controlling task execution flow,
//! including success/failure handling, waiting, and aborting tasks based on tags.

use std::convert::Infallible;
use std::time::Duration;

use crate::context::Tag;
use crate::{BoxError, Error};

/// Defines a way to select or filter [`Tag`]s for flow control operations.
///
/// Used with [`FlowControl`] variants like [`FlowControl::Wait`], [`FlowControl::Hold`], and [`FlowControl::Fail`]
/// to specify which tagged requests should be affected by the flow control.
///
/// # Examples
///
/// ```
/// use spire_core::context::{Tag, TagQuery};
/// use std::time::Duration;
///
/// // Match only the current request's tag
/// let query = TagQuery::Owner;
///
/// // Match a specific tag
/// let query = TagQuery::Single(Tag::from("product_list"));
///
/// // Match multiple tags
/// let query = TagQuery::List(vec![Tag::from("page1"), Tag::from("page2")]);
///
/// // Match all tags
/// let query = TagQuery::Every;
/// ```
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
    pub fn is_match(&self, tag: &Tag, owner: &Tag) -> bool {
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
pub enum FlowControl {
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

impl FlowControl {
    /// Creates a new [`FlowControl`] from the boxable error.
    pub fn error(error: impl Into<BoxError>) -> Self {
        error.into().into_flow_control()
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

/// Trait for generating [`FlowControl`]s.
pub trait IntoFlowControl {
    /// Transforms `self` into the [`FlowControl`].
    fn into_flow_control(self) -> FlowControl;
}

impl IntoFlowControl for FlowControl {
    #[inline]
    fn into_flow_control(self) -> FlowControl {
        self
    }
}

impl IntoFlowControl for () {
    #[inline]
    fn into_flow_control(self) -> FlowControl {
        FlowControl::Continue
    }
}

impl IntoFlowControl for Infallible {
    #[inline]
    fn into_flow_control(self) -> FlowControl {
        FlowControl::Continue
    }
}

impl IntoFlowControl for Duration {
    #[inline]
    fn into_flow_control(self) -> FlowControl {
        FlowControl::Wait(TagQuery::default(), self)
    }
}

impl IntoFlowControl for BoxError {
    #[inline]
    fn into_flow_control(self) -> FlowControl {
        Error::from_boxed(self).into_flow_control()
    }
}

impl<T> IntoFlowControl for Option<T>
where
    T: IntoFlowControl,
{
    fn into_flow_control(self) -> FlowControl {
        self.map_or_else(
            || ().into_flow_control(),
            IntoFlowControl::into_flow_control,
        )
    }
}

impl<T, E> IntoFlowControl for Result<T, E>
where
    T: IntoFlowControl,
    E: IntoFlowControl,
{
    fn into_flow_control(self) -> FlowControl {
        fn flip(x: FlowControl) -> FlowControl {
            match x {
                FlowControl::Continue => FlowControl::Skip,
                FlowControl::Skip => FlowControl::Continue,
                FlowControl::Wait(q, x) => FlowControl::Hold(q, x),
                FlowControl::Hold(q, x) => FlowControl::Wait(q, x),
                FlowControl::Fail(q, x) => FlowControl::Fail(q, x),
            }
        }

        match self {
            Ok(x) => x.into_flow_control(),
            Err(x) => flip(x.into_flow_control()),
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::context::{FlowControl, IntoFlowControl};

    #[test]
    fn basic() {
        assert!(matches!(().into_flow_control(), FlowControl::Continue));
        let data = Duration::default();
        assert!(matches!(data.into_flow_control(), FlowControl::Wait(..)));
    }

    #[test]
    fn with_option() {
        let flip: Option<Duration> = Some(Duration::default());
        assert!(matches!(flip.into_flow_control(), FlowControl::Wait(..)));
        let flip: Option<Duration> = None;
        assert!(matches!(flip.into_flow_control(), FlowControl::Continue));
    }

    #[test]
    fn with_result() {
        let flip: Result<Duration, Duration> = Ok(Duration::default());
        assert!(matches!(flip.into_flow_control(), FlowControl::Wait(..)));
        let flip: Result<Duration, Duration> = Err(Duration::default());
        assert!(matches!(flip.into_flow_control(), FlowControl::Hold(..)));
    }
}
