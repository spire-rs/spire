use std::num::NonZeroUsize;

use crate::context::Request;

/// Defines the [`Request`] identifier used for routing.
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

#[derive(Debug, Copy, Clone)]
pub struct Depth(pub NonZeroUsize);

impl Depth {
    pub fn new(depth: usize) -> Self {
        let depth = NonZeroUsize::new(depth);
        depth.map(Self).unwrap_or_default()
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

impl From<Depth> for NonZeroUsize {
    fn from(value: Depth) -> Self {
        value.0
    }
}

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

pub trait Task {}

impl<B> Task for Request<B> {}
