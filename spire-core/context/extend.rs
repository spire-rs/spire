use std::num::NonZeroUsize;

use http::request::Builder;

use crate::context::Request;

/// Extends the [`Request`] with an identifier used for routing.
///
/// To ensure the type-safe usage of [`Tag`]s and [`Task`]s inside of handlers,
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
    /// The smallest recursive [`Depth`] value.
    const MIN: Depth = Depth(NonZeroUsize::MIN);

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
        Depth::MIN
    }
}

/// Extends a [`Request`] and [`Response`] with event timestamps.
///
/// [`Request`]: http::Request
/// [`Response`]: http::Response
#[derive(Debug, Clone)]
pub struct Time {
    // TODO: Timing<BetweenReqResp>, <BeforeResp>, <SinceReq>, <SinceResp>
    // Req created, Handler called, Resp created
    initialized: (),
    dispatched: (),
}

impl Default for Time {
    fn default() -> Self {
        todo!()
    }
}

mod sealed {
    use super::{Builder, Request};

    pub trait Sealed {}
    impl<B> Sealed for Request<B> {}
    impl Sealed for Builder {}
}

/// Extension trait for `http::`[`Request`].
pub trait Task: sealed::Sealed {
    // TODO: Event timestamps.

    /// Returns a reference to the attached tag.
    fn tag(&self) -> Option<&Tag>;
    /// Returns a mutable reference to the attached tag.
    fn tag_mut(&mut self) -> Option<&mut Tag>;
    /// Returns a recursive depth of this [`Request`].
    fn depth(&self) -> usize;
}

impl<B> Task for Request<B> {
    fn tag(&self) -> Option<&Tag> {
        self.extensions().get()
    }

    fn tag_mut(&mut self) -> Option<&mut Tag> {
        self.extensions_mut().get_mut()
    }

    fn depth(&self) -> usize {
        let depth = self.extensions().get::<Depth>();
        depth.unwrap_or(&Depth::MIN).get()
    }
}

/// Extension trait for `http::request::`[`Builder`].
pub trait TaskBuilder {
    /// Attaches a [`Tag`] to this [`Builder`].
    fn tag(self, tag: impl Into<Tag>) -> Self;

    /// Attaches a depth value to this [`Builder`].
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

    use crate::context::{Body, Tag, TaskBuilder};

    #[test]
    fn build() {
        let build = Builder::new()
            .uri("https://example.com/")
            .tag(Tag::default())
            .body(Body::default());
        matches!(build, Ok(_));
    }
}
