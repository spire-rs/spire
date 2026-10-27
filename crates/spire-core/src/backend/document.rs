//! Optional link observations from a rendered document.

use ::http::Uri;
use futures::future::BoxFuture;

use crate::Result;

/// A live rendered document, separate from a fetched response snapshot.
///
/// Methods return boxed futures to make this optional capability object
/// safe. Ordinary HTTP sessions do not implement it or allocate these
/// futures. Captures observe the current DOM and must not scroll or mutate it.
pub trait Document: Send + Sync {
    /// Captures links and their visibility together, in document order.
    /// `selector` narrows `<a href>` and `<area href>` links using CSS.
    ///
    /// # Errors
    ///
    /// Returns an error if the selector is invalid, no page has been
    /// fetched, or the document cannot be read.
    fn links<'a>(&'a self, selector: Option<&'a str>) -> BoxFuture<'a, Result<DocumentLinks>>;
}

/// What a session knows about whether a visitor can follow a link.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Visibility {
    /// The rendered link has a usable area and is not hidden or clipped.
    Visible,
    /// Attributes, styles or layout hide the link.
    Hidden,
    /// The session cannot establish visibility, e.g. without CSS and layout.
    #[default]
    Unknown,
}

/// The element a document link comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LinkKind {
    /// An `<a href>`.
    Anchor,
    /// An `<area href>` of an image map.
    Area,
}

/// A link and its visibility, observed together from one rendered document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentLink {
    /// The `href` as written in the document.
    pub href: String,
    /// The link's `rel` tokens.
    pub rel: Vec<String>,
    /// The anchor's text, or an area's `alt` text.
    pub text: String,
    /// The element the link comes from.
    pub kind: LinkKind,
    /// Visibility at the time the link was observed.
    pub visibility: Visibility,
}

/// Links captured together from a rendered document, in document order.
///
/// The page URI, base URI, link attributes and visibility must all come
/// from the same DOM capture. Duplicate URLs are separate link records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentLinks {
    /// The document's current URI, after navigation or script changes.
    pub uri: Uri,
    /// The document's base URI, including its `<base href>`, if any.
    pub base_uri: String,
    /// The observed links, including hidden and unknown links.
    pub links: Vec<DocumentLink>,
}
