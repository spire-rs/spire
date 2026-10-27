//! Requests and their crawl metadata.

use bytes::Bytes;

use super::meta::RequestMeta;
use crate::crawler::{Attempt, Depth, Tag};
use crate::identity::Identity;

/// An HTTP request, with its body in memory.
pub type Request<B = Bytes> = ::http::Request<B>;

/// Reads crawl metadata from a [`Request`].
///
/// A request gets its [`Tag`], [`Depth`] and [`Attempt`] from the seed it is
/// made from, in [`Seed::into_request`]. A request built any other way has
/// no tag, the root depth and the first attempt; to give one metadata, e.g.
/// to test a handler, build it from a seed:
///
/// ```
/// use spire_core::frontier::seed::Seed;
/// use spire_core::http::RequestExt;
/// use spire_core::crawler::Tag;
///
/// let request = Seed::get("https://example.com/")?.with_tag("page").into_request();
/// assert_eq!(request.tag(), Some(&Tag::from("page")));
/// # Ok::<_, spire_core::Error>(())
/// ```
///
/// [`Seed::into_request`]: crate::frontier::seed::Seed::into_request
pub trait RequestExt {
    /// Returns the tag, if the request has one.
    fn tag(&self) -> Option<&Tag>;

    /// Returns the depth, defaulting to [`Depth::ROOT`].
    fn depth(&self) -> Depth;

    /// Returns the attempt, defaulting to [`Attempt::FIRST`].
    fn attempt(&self) -> Attempt;

    /// Returns the identity the request goes out as, if the crawl has
    /// [identities](crate::identity).
    fn identity(&self) -> Option<&Identity>;
}

impl<B> RequestExt for ::http::Request<B> {
    fn tag(&self) -> Option<&Tag> {
        self.extensions().get::<RequestMeta>()?.tag.as_ref()
    }

    fn depth(&self) -> Depth {
        self.extensions()
            .get::<RequestMeta>()
            .map(|meta| meta.depth)
            .unwrap_or_default()
    }

    fn attempt(&self) -> Attempt {
        self.extensions()
            .get::<RequestMeta>()
            .map(|meta| meta.attempt)
            .unwrap_or_default()
    }

    fn identity(&self) -> Option<&Identity> {
        self.extensions().get::<RequestMeta>()?.identity.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_metadata_defaults() {
        let request = Request::new(());
        assert_eq!(request.tag(), None);
        assert_eq!(request.depth(), Depth::ROOT);
        assert_eq!(request.attempt(), Attempt::FIRST);
    }

    #[test]
    fn request_metadata_from_extensions() {
        let mut request = Request::new(());
        request.extensions_mut().insert(RequestMeta {
            tag: Some(Tag::from_static("page")),
            depth: Depth::new(2),
            attempt: Attempt::FIRST,
            identity: None,
        });
        assert_eq!(request.tag(), Some(&Tag::from("page")));
        assert_eq!(request.depth().child(), Depth::new(3));
    }
}
