use std::num::NonZeroUsize;

use crate::context::{Body, Request};

/// Extends the [`Request`] with an identifier used for routing.
///
/// To ensure the type-safe usage of [`Tag`]s and [`crate::context::Task`]s inside of handlers,
/// you may want to create a custom enum, that implements `Into<Tag>` or `Into<Task>`:
///
/// ```rust
/// use spire_core::context::Tag;
///
/// #[derive(Debug, Clone)]
/// pub enum Routes {
///     DiscoverLinks(String),
///     ExtractFromPage(String),
/// }
///
/// impl Into<Tag> for Routes {
///     fn into(self) -> Tag {
///         match self {
///             Routes::DiscoverLinks(x) => todo!(),
///             Routes::ExtractFromPage(x) => todo!(),
///         }
///     }
/// }
/// ```
#[derive(Debug, Clone, Hash, Eq, PartialEq, Default)]
pub enum Tag {
    /// Explicitly call the fallback handler.
    #[default]
    Fallback,
    Sequence(String),
    Rehash(u64),
}

impl From<&str> for Tag {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl From<String> for Tag {
    fn from(value: String) -> Self {
        Tag::Sequence(value)
    }
}

impl From<u64> for Tag {
    fn from(value: u64) -> Self {
        Tag::Rehash(value)
    }
}

/// Extends a [`Request`] to track a recursively increasing depth.
#[derive(Debug, Copy, Clone)]
pub struct Depth(pub NonZeroUsize);

impl Depth {
    /// Creates a new [`Depth`] extension.
    pub fn new(depth: usize) -> Self {
        Self(NonZeroUsize::new(depth).unwrap_or(NonZeroUsize::MIN))
    }

    /// Returns the depth as a primitive type.
    pub fn get(&self) -> usize {
        self.0.get()
    }
}

impl Default for Depth {
    fn default() -> Self {
        NonZeroUsize::MIN.into()
    }
}

impl From<NonZeroUsize> for Depth {
    fn from(value: NonZeroUsize) -> Self {
        Depth(value)
    }
}

/// Extends a [`Request`] and [`Response`] with event timestamps.
///
/// [`Request`]: http::Request
/// [`Response`]: http::Response
#[derive(Debug, Clone)]
pub struct Time {
    initialized: (),
    dispatched: (),
}

impl Default for Time {
    fn default() -> Self {
        todo!()
    }
}

pub trait Task {
    // Replace tag, get event timestamps, get depth
}

impl Task for Request<Body> {}

#[cfg(test)]
mod test {

    #[test]
    fn with_tag() {}

    #[test]
    fn with_depth() {}

    #[test]
    fn with_time() {}
}
