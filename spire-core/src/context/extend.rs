use std::num::NonZeroUsize;

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
    Sequence(String),
    /// Unique identifier used for routing.
    Rehash(u64),
}

impl Tag {
    /// Returns `true` if the [`Tag`] is an explicit fallback.
    #[must_use]
    pub const fn is_fallback(&self) -> bool {
        matches!(self, Self::Fallback)
    }
}

impl From<&str> for Tag {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl From<String> for Tag {
    fn from(value: String) -> Self {
        Self::Sequence(value)
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
    pub fn new(depth: usize) -> Self {
        Self(NonZeroUsize::new(depth).unwrap_or(NonZeroUsize::MIN))
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

/// Extension trait for `http::`[`Request`].
pub trait Task {
    /// Returns a reference to the attached [`Tag`].
    fn try_tag(&self) -> Option<&Tag>;

    /// Returns a reference to the attached [`Tag`], [`Tag::Fallback`] otherwise.
    fn tag(&self) -> &Tag;

    /// Returns a recursive depth of this [`Request`].
    fn depth(&self) -> usize;
}

impl<B> Task for Request<B> {
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

    use crate::context::{Body, Request, Tag, Task, TaskBuilder};

    fn make_request(f: fn(Builder) -> Builder) -> Request {
        let request = Builder::new().uri("https://example.com/");
        f(request).body(Body::default()).unwrap()
    }

    #[test]
    fn with_tag() {
        let request = make_request(|x| x);
        assert_eq!(request.tag(), &Tag::Fallback);
        let request = make_request(|x| x.tag(""));
        assert_eq!(request.tag(), &"".into());
    }

    #[test]
    fn with_depth() {
        let request = make_request(|x| x);
        assert_eq!(request.depth(), 1);
        let request = make_request(|x| x.depth(2));
        assert_eq!(request.depth(), 2);
    }
}
