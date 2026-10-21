use http::Uri;

use crate::frontier::Seed;
use crate::request::Tag;

/// A link to follow with [`Queue::follow`](crate::frontier::Queue::follow).
///
/// A link is either an `href`, absolute or relative to the page it was found
/// on, or a complete [`Seed`]. Strings, URIs and seeds convert into links.
///
/// ```
/// use spire_core::frontier::Link;
///
/// let next = Link::from("/page/2");
/// let detail = Link::from("../items/42").with_tag("item");
/// ```
#[derive(Debug, Clone)]
pub struct Link {
    pub(crate) target: Target,
    pub(crate) tag: Option<Tag>,
}

#[derive(Debug, Clone)]
pub(crate) enum Target {
    Href(String),
    Seed(Box<Seed>),
}

impl Link {
    /// Creates a link from an `href`, absolute or relative.
    #[must_use]
    pub fn new(href: impl Into<String>) -> Self {
        Self {
            target: Target::Href(href.into()),
            tag: None,
        }
    }

    /// Routes the followed request to the handler for `tag`.
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<Tag>) -> Self {
        self.tag = Some(tag.into());
        self
    }
}

impl From<&str> for Link {
    fn from(href: &str) -> Self {
        Self::new(href)
    }
}

impl From<String> for Link {
    fn from(href: String) -> Self {
        Self::new(href)
    }
}

impl From<&String> for Link {
    fn from(href: &String) -> Self {
        Self::new(href.as_str())
    }
}

impl From<Uri> for Link {
    fn from(uri: Uri) -> Self {
        Self::new(uri.to_string())
    }
}

impl From<Seed> for Link {
    fn from(seed: Seed) -> Self {
        Self {
            target: Target::Seed(Box::new(seed)),
            tag: None,
        }
    }
}
