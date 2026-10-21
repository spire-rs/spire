//! Crawl metadata attached to requests and responses.

use std::borrow::Cow;
use std::fmt;

use http::Uri;

use crate::request::{Request, Response};

/// A label that routes a request to its handler.
///
/// Requests without a tag go to the fallback handler.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tag(Cow<'static, str>);

impl Tag {
    /// Creates a tag from a static string without allocating.
    #[must_use]
    pub const fn from_static(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }

    /// Returns the tag as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Tag {
    fn from(value: &str) -> Self {
        Self(Cow::Owned(value.to_owned()))
    }
}

impl From<String> for Tag {
    fn from(value: String) -> Self {
        Self(Cow::Owned(value))
    }
}

impl AsRef<str> for Tag {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A closed set of [`Tag`]s, typically an enum of the kinds of page a crawl
/// visits.
///
/// Deriving it with `#[derive(Tags)]` (the `macros` feature of `spire`)
/// names each unit variant in `snake_case`, so tags are checked by the
/// compiler while plain string tags keep working:
///
/// - `#[tag(rename_all = "...")]` on the enum picks another case, with
///   serde's names: `lowercase`, `UPPERCASE`, `PascalCase`, `camelCase`,
///   `snake_case`, `SCREAMING_SNAKE_CASE`, `kebab-case` or
///   `SCREAMING-KEBAB-CASE`.
/// - `#[tag(rename = "...")]` on a variant names it outright.
///
/// Persistent frontiers store tags as strings: renaming a tag strands the
/// seeds stored under its old name, unless `rename` keeps the old one.
///
/// Implemented by hand, it looks like this:
///
/// ```
/// use spire_core::request::{Tag, Tags};
///
/// #[derive(Debug, Clone, Copy, PartialEq)]
/// enum Page {
///     List,
///     Item,
/// }
///
/// impl Page {
///     const fn as_str(self) -> &'static str {
///         match self {
///             Self::List => "list",
///             Self::Item => "item",
///         }
///     }
/// }
///
/// impl From<Page> for Tag {
///     fn from(page: Page) -> Self {
///         Tag::from_static(page.as_str())
///     }
/// }
///
/// impl Tags for Page {
///     const ALL: &'static [Self] = &[Self::List, Self::Item];
/// }
///
/// assert_eq!(Page::from_tag(&Tag::from("item")), Some(Page::Item));
/// assert_eq!(Page::from_tag(&Tag::from("other")), None);
/// ```
pub trait Tags: Copy + Into<Tag> + 'static {
    /// Every tag in the set.
    const ALL: &'static [Self];

    /// Returns the member named `tag`, if any.
    #[must_use]
    fn from_tag(tag: &Tag) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|member| (*member).into() == *tag)
    }
}

/// How many links away from a seed a request is.
///
/// Seeds have depth 0; every followed link is one deeper than its parent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Depth(u32);

impl Depth {
    /// The depth of a seed.
    pub const ROOT: Self = Self(0);

    /// Creates a depth.
    #[must_use]
    pub const fn new(depth: u32) -> Self {
        Self(depth)
    }

    /// Returns the depth as a number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Returns the depth of a link followed from this one.
    #[must_use]
    pub const fn child(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for Depth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// How many times a request was tried before.
///
/// The first try is attempt 0.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Attempt(u32);

impl Attempt {
    /// The first try.
    pub const FIRST: Self = Self(0);

    /// Creates an attempt.
    #[must_use]
    pub const fn new(attempt: u32) -> Self {
        Self(attempt)
    }

    /// Returns the attempt as a number.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Returns the attempt after this one.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl fmt::Display for Attempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Reads crawl metadata from a [`Request`].
///
/// The crawler stores the [`Tag`], [`Depth`] and [`Attempt`] of every request
/// in its extensions.
pub trait RequestExt {
    /// Returns the tag, if the request has one.
    fn tag(&self) -> Option<&Tag>;

    /// Returns the depth, defaulting to [`Depth::ROOT`].
    fn depth(&self) -> Depth;

    /// Returns the attempt, defaulting to [`Attempt::FIRST`].
    fn attempt(&self) -> Attempt;
}

impl<B> RequestExt for Request<B> {
    fn tag(&self) -> Option<&Tag> {
        self.extensions().get()
    }

    fn depth(&self) -> Depth {
        self.extensions().get().copied().unwrap_or_default()
    }

    fn attempt(&self) -> Attempt {
        self.extensions().get().copied().unwrap_or_default()
    }
}

/// The URI a response was finally served from, after redirects.
#[derive(Debug, Clone)]
struct FinalUri(Uri);

/// Reads and writes crawl metadata on a [`Response`].
pub trait ResponseExt {
    /// Returns the URI the response was served from after redirects, if the
    /// backend recorded it.
    ///
    /// Resolve relative links against this rather than the request URI.
    fn final_uri(&self) -> Option<&Uri>;

    /// Records the URI the response was served from; called by backends.
    fn set_final_uri(&mut self, uri: Uri);
}

impl<B> ResponseExt for Response<B> {
    fn final_uri(&self) -> Option<&Uri> {
        self.extensions().get::<FinalUri>().map(|FinalUri(uri)| uri)
    }

    fn set_final_uri(&mut self, uri: Uri) {
        self.extensions_mut().insert(FinalUri(uri));
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
    fn response_final_uri() {
        let mut response = Response::new(());
        assert_eq!(response.final_uri(), None);
        response.set_final_uri(Uri::from_static("https://example.com/moved"));
        assert_eq!(response.final_uri().unwrap(), "https://example.com/moved");
    }

    #[test]
    fn request_metadata_from_extensions() {
        let mut request = Request::new(());
        request.extensions_mut().insert(Tag::from_static("page"));
        request.extensions_mut().insert(Depth::new(2));
        assert_eq!(request.tag(), Some(&Tag::from("page")));
        assert_eq!(request.depth().child(), Depth::new(3));
    }
}
