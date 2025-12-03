//! Request extensions for routing and depth tracking.
//!
//! This module provides extensions to HTTP requests for managing routing tags
//! and tracking request depth in recursive scraping scenarios.

use std::borrow::Cow;
use std::num::NonZeroUsize;

use derive_more::{Deref, DerefMut};
use http::request::Builder;

use crate::context::Request;

/// Extends the [`Request`] with an identifier used for routing.
///
/// To ensure the type-safe usage of [`Tag`]s inside of handlers,
/// you may want to create a custom enum, that implements `Into<Tag>`:
///
/// ```rust
/// # use spire_core::context::Tag;
///
/// #[derive(Debug, Clone)]
/// pub enum Routes {
///     A,
///     B,
/// }
///
/// impl From<Routes> for Tag {
///     fn from(route: Routes) -> Self {
///         match route {
///             // ...
/// #           Routes::A => 1.into(),
/// #           Routes::B => 2.into(),
///         }
///     }
/// }
///
/// ```
#[derive(Debug, Clone, Hash, Eq, PartialEq, Default)]
pub enum Tag {
    /// Explicitly calls the fallback handler.
    #[default]
    Fallback,
    /// Unique identifier used for routing.
    Sequence(Cow<'static, str>),
    /// Unique identifier used for routing.
    Rehash(u64),
}

impl Tag {
    /// Returns `true` if the [`Tag`] is an explicit fallback.
    #[must_use]
    pub const fn is_fallback(&self) -> bool {
        matches!(self, Self::Fallback)
    }

    /// Creates a new [`Tag`] from a static string slice.
    pub const fn from_static(value: &'static str) -> Self {
        Self::Sequence(Cow::Borrowed(value))
    }
}

impl From<&str> for Tag {
    fn from(value: &str) -> Self {
        Self::Sequence(Cow::Owned(value.to_owned()))
    }
}

impl From<String> for Tag {
    fn from(value: String) -> Self {
        Self::Sequence(Cow::Owned(value))
    }
}

impl From<u64> for Tag {
    fn from(value: u64) -> Self {
        Self::Rehash(value)
    }
}

/// Extends a [`Request`] to track a recursively increasing depth.
#[derive(Debug, Copy, Clone)]
pub struct Depth(pub NonZeroUsize);

impl Depth {
    /// The smallest recursive [`Depth`] value.
    const MIN: Self = Self(NonZeroUsize::MIN);

    /// Creates a new [`Depth`] extension.
    ///
    /// If `depth` is 0, uses the minimum value (1) instead.
    pub fn new(depth: usize) -> Self {
        Self(NonZeroUsize::new(depth).unwrap_or_else(|| {
            debug_assert!(false, "Depth::new called with 0, using MIN instead");
            NonZeroUsize::MIN
        }))
    }

    /// Returns the depth as a primitive type.
    pub const fn get(self) -> usize {
        self.0.get()
    }
}

impl Default for Depth {
    fn default() -> Self {
        Self::MIN
    }
}

/// Wrapper around `http::`[`Request`] with additional functionality.
#[derive(Deref, DerefMut)]
pub struct Task {
    #[deref]
    #[deref_mut]
    inner: Request,
}

impl Task {
    /// Creates a new Task from a Request.
    pub fn new(request: Request) -> Self {
        Self { inner: request }
    }

    /// Returns a reference to the attached [`Tag`].
    pub fn try_tag(&self) -> Option<&Tag> {
        self.extensions().get()
    }

    /// Returns a reference to the attached [`Tag`], [`Tag::Fallback`] otherwise.
    pub fn tag(&self) -> &Tag {
        self.try_tag().unwrap_or(&Tag::Fallback)
    }

    /// Returns a recursive depth of this [`Request`].
    pub fn depth(&self) -> usize {
        let depth = self.extensions().get::<Depth>();
        depth.unwrap_or(&Depth::MIN).get()
    }
}

impl From<Request> for Task {
    fn from(request: Request) -> Self {
        Self::new(request)
    }
}

impl std::fmt::Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.method(), self.uri())
    }
}

/// Extension trait for `http::`[`Request`] to provide Task functionality.
pub trait TaskExt {
    /// Returns a reference to the attached [`Tag`].
    fn try_tag(&self) -> Option<&Tag>;

    /// Returns a reference to the attached [`Tag`], [`Tag::Fallback`] otherwise.
    fn tag(&self) -> &Tag;

    /// Returns a recursive depth of this [`Request`].
    fn depth(&self) -> usize;
}

impl<B> TaskExt for Request<B> {
    fn try_tag(&self) -> Option<&Tag> {
        self.extensions().get()
    }

    fn tag(&self) -> &Tag {
        self.try_tag().unwrap_or(&Tag::Fallback)
    }

    fn depth(&self) -> usize {
        let depth = self.extensions().get::<Depth>();
        depth.unwrap_or(&Depth::MIN).get()
    }
}

/// Extension trait for `http::request::`[`Builder`].
pub trait TaskBuilder {
    /// Attaches a [`Tag`] to this [`Builder`].
    #[must_use]
    fn tag(self, tag: impl Into<Tag>) -> Self;

    /// Attaches a depth value to this [`Builder`].
    #[must_use]
    fn depth(self, depth: usize) -> Self;
}

impl TaskBuilder for Builder {
    fn tag(self, tag: impl Into<Tag>) -> Self {
        self.extension(tag.into())
    }

    fn depth(self, depth: usize) -> Self {
        self.extension(Depth::new(depth))
    }
}

#[cfg(test)]
mod test {
    use http::request::Builder;

    use crate::context::{Body, Tag, Task, TaskBuilder, TaskExt};

    fn make_task(f: fn(Builder) -> Builder) -> Task {
        let request = Builder::new().uri("https://example.com/");
        let request = f(request).body(Body::default()).unwrap();
        Task::new(request)
    }

    #[test]
    fn with_tag() {
        let task = make_task(|x| x);
        assert_eq!(task.tag(), &Tag::Fallback);
        let task = make_task(|x| x.tag(""));
        assert_eq!(task.tag(), &"".into());
    }

    #[test]
    fn with_depth() {
        let task = make_task(|x| x);
        assert_eq!(task.depth(), 1);
        let task = make_task(|x| x.depth(2));
        assert_eq!(task.depth(), 2);
    }

    #[test]
    fn task_ext_on_request() {
        let request = Builder::new()
            .uri("https://example.com/")
            .tag("test")
            .body(Body::default())
            .unwrap();
        assert_eq!(request.tag(), &Tag::from("test"));
        assert_eq!(request.depth(), 1);
    }
}
