use http::Uri;

use crate::frontier::seed::Seed;
use crate::request::Tag;

/// A link to follow with [`Queue::follow`](crate::frontier::queue::Queue::follow).
///
/// A link is either an `href`, absolute or relative to the page it was found
/// on, or a complete [`Seed`]. Strings, URIs and seeds convert into links.
///
/// ```
/// use spire_core::frontier::queue::Link;
///
/// let next = Link::from("/page/2");
/// let detail = Link::from("../items/42").with_tag("item");
/// let next = Link::from("/page/3").with_priority(10);
/// ```
#[derive(Debug, Clone)]
pub struct Link {
    pub(crate) target: Target,
    pub(crate) tag: Option<Tag>,
    pub(crate) priority: Option<i32>,
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
            priority: None,
        }
    }

    /// Routes the followed request to the handler for `tag`.
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<Tag>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Sets the followed request's priority, e.g. scored from the page the
    /// link was found on; frontiers that order by priority crawl higher
    /// priorities first. A [scoring](crate::frontier::FrontierExt::score)
    /// frontier adds its score to it.
    #[must_use]
    pub const fn with_priority(mut self, priority: i32) -> Self {
        self.priority = Some(priority);
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
            priority: None,
        }
    }
}
