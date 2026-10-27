//! Queues: [`Queue`], the handle handlers add links through, and [`Link`].

use std::fmt;
use std::sync::{Arc, Mutex};

use ::http::Uri;
use url::Url;

mod link;

pub use link::Link;
use link::Target;

use crate::crawler::Depth;
use crate::frontier::dynamic::DynFrontier;
use crate::frontier::seed::Seed;
use crate::frontier::{Admission, Frontier};
use crate::http::{Request, RequestExt};
use crate::sync::lock;
use crate::{Error, Result};

/// A cloneable handle for adding seeds to a crawl's [`Frontier`].
///
/// Handlers get a queue bound to the request they handle, so
/// [`Queue::follow`] can resolve relative links and derive the depth of the
/// requests it queues.
#[derive(Clone)]
pub struct Queue {
    frontier: Arc<dyn DynFrontier>,
    parent: Option<Arc<Parent>>,
}

struct Parent {
    depth: Depth,
    /// The URI relative links resolve against: the request URI, replaced by
    /// the final URI once the response is fetched.
    base: Mutex<Uri>,
}

impl Queue {
    /// Creates a queue over `frontier`.
    #[must_use]
    pub fn new(frontier: impl Frontier) -> Self {
        Self::from_dyn(Arc::new(frontier))
    }

    pub(crate) fn from_dyn(frontier: Arc<dyn DynFrontier>) -> Self {
        Self {
            frontier,
            parent: None,
        }
    }

    /// Returns a queue whose [`follow`](Self::follow) treats links as found
    /// on `request`.
    #[must_use]
    pub fn child_of<B>(&self, request: &Request<B>) -> Self {
        let parent = Parent {
            depth: request.depth(),
            base: Mutex::new(request.uri().clone()),
        };

        Self {
            frontier: self.frontier.clone(),
            parent: Some(Arc::new(parent)),
        }
    }

    /// Resolves relative links against `uri` from now on, e.g. the URI a
    /// response was served from after redirects. Clones share the base.
    pub fn rebase(&self, uri: Uri) {
        if let Some(parent) = &self.parent {
            *lock(&parent.base) = uri;
        }
    }

    /// Adds `seed` as is.
    ///
    /// # Errors
    ///
    /// Returns an error if `seed` cannot be converted into a [`Seed`] or the
    /// frontier fails.
    pub async fn push<S>(&self, seed: S) -> Result<Admission>
    where
        S: TryInto<Seed>,
        S::Error: Into<Error>,
    {
        let seed = seed.try_into().map_err(Into::into)?;
        self.frontier.push(seed).await
    }

    /// Adds a link found on the current request, one level deeper.
    ///
    /// The followed request has the link's tag and priority, or else the
    /// seed's. Tags are not inherited: an untagged link goes to the router's
    /// path routes or fallback, so pagination tags its links explicitly.
    ///
    /// The tag is not part of the [`Fingerprint`](crate::frontier::seed::Fingerprint):
    /// with a deduplicating frontier, a URL already followed under one tag is
    /// a [`Duplicate`](Admission::Duplicate) under any other.
    ///
    /// Relative `href`s resolve against the page; fragments are dropped.
    /// Links that are not `http(s)` URLs, such as `mailto:` or malformed
    /// ones, are skipped with [`Admission::Unsupported`]. Outside a handler,
    /// links must be absolute and keep their depth.
    ///
    /// # Errors
    ///
    /// Returns an error if the frontier fails.
    pub async fn follow(&self, link: impl Into<Link>) -> Result<Admission> {
        let link = link.into();
        let mut seed = match link.target {
            Target::Seed(seed) => *seed,
            Target::Href(href) => match self.resolve(&href) {
                Some(seed) => seed,
                None => return Ok(Admission::Unsupported),
            },
        };

        if let Some(tag) = link.tag {
            seed = seed.with_tag(tag);
        }
        if let Some(priority) = link.priority {
            seed = seed.with_priority(priority);
        }
        if let Some(parent) = &self.parent {
            seed = seed.with_depth(parent.depth.child());
        }
        self.frontier.push(seed).await
    }

    /// Follows every link, returning how many were queued.
    ///
    /// # Errors
    ///
    /// Returns an error if the frontier fails.
    pub async fn follow_all<I>(&self, links: I) -> Result<usize>
    where
        I: IntoIterator<Item: Into<Link>>,
    {
        let mut queued = 0;
        for link in links {
            if self.follow(link).await?.is_queued() {
                queued += 1;
            }
        }
        Ok(queued)
    }

    /// Resolves `href` into a `GET` seed, or `None` if it is not a crawlable
    /// `http(s)` URL.
    fn resolve(&self, href: &str) -> Option<Seed> {
        let href = href.trim();
        let mut url = match &self.parent {
            Some(parent) => {
                let base = lock(&parent.base).to_string();
                Url::parse(&base).ok()?.join(href).ok()?
            }
            None => Url::parse(href).ok()?,
        };

        if !matches!(url.scheme(), "http" | "https") {
            return None;
        }
        url.set_fragment(None);
        Seed::get(url.as_str()).ok()
    }

    /// Returns the number of queued seeds, if known.
    #[must_use]
    pub fn size_hint(&self) -> Option<usize> {
        self.frontier.size_hint()
    }
}

impl fmt::Debug for Queue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let base = self
            .parent
            .as_ref()
            .map(|parent| lock(&parent.base).clone());
        f.debug_struct("Queue")
            .field("base", &base)
            .field("size_hint", &self.size_hint())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawler::Tag;
    use crate::frontier::{FrontierExt, InMemFrontier, Pop};

    async fn drain(frontier: &InMemFrontier) -> Vec<(String, Option<String>, u32)> {
        let mut popped = Vec::new();
        while let Pop::Ready(seed) = Frontier::pop(frontier).await.unwrap() {
            let tag = seed.tag().map(Tag::to_string);
            popped.push((seed.uri().to_string(), tag, seed.depth().get()));
        }
        popped
    }

    fn page(uri: &str) -> Request<()> {
        let seed = Seed::get(uri)
            .unwrap()
            .with_tag("list")
            .with_depth(Depth::new(2));
        seed.into_request().map(drop)
    }

    #[tokio::test]
    async fn follow_derives_depth_not_tag() {
        let frontier = Arc::new(InMemFrontier::fifo());
        let queue = Queue::from_dyn(frontier.clone());
        let child = queue.child_of(&page("https://example.com/"));

        child.follow("https://example.com/untagged").await.unwrap();
        child
            .follow(Link::from("/own").with_tag("detail"))
            .await
            .unwrap();
        let seed = Seed::get("https://example.com/seed")
            .unwrap()
            .with_tag("seed");
        child.follow(seed).await.unwrap();
        queue.follow("https://example.com/root").await.unwrap();

        let tag = |tag: &str| Some(tag.to_owned());
        assert_eq!(
            drain(&frontier).await,
            [
                ("https://example.com/untagged".to_owned(), None, 3),
                ("https://example.com/own".to_owned(), tag("detail"), 3),
                ("https://example.com/seed".to_owned(), tag("seed"), 3),
                ("https://example.com/root".to_owned(), None, 0),
            ]
        );
    }

    #[tokio::test]
    async fn follow_sets_the_link_priority() {
        let frontier = Arc::new(InMemFrontier::priority());
        let child = Queue::from_dyn(frontier.clone()).child_of(&page("https://example.com/"));

        child.follow("/plain").await.unwrap();
        child
            .follow(Link::from("/scored").with_priority(7))
            .await
            .unwrap();
        let seed = Seed::get("https://example.com/seed")
            .unwrap()
            .with_priority(3);
        child.follow(seed).await.unwrap();

        let mut priorities: Vec<_> = Frontier::snapshot(&*frontier)
            .unwrap()
            .iter()
            .map(|seed| (seed.uri().path().to_owned(), seed.priority()))
            .collect();
        priorities.sort_by_key(|(_, priority)| -priority);
        assert_eq!(
            priorities,
            [
                ("/scored".to_owned(), 7),
                ("/seed".to_owned(), 3),
                ("/plain".to_owned(), 0),
            ]
        );
    }

    #[tokio::test]
    async fn follow_resolves_relative_links() {
        let frontier = Arc::new(InMemFrontier::fifo());
        let child = Queue::from_dyn(frontier.clone()).child_of(&page("https://example.com/a/b"));

        for href in ["c", "../d", "/e?x=1", "//other.org/f", "g#section", " h "] {
            assert!(child.follow(href).await.unwrap().is_queued(), "{href}");
        }
        let resolved: Vec<_> = drain(&frontier).await.into_iter().map(|x| x.0).collect();
        assert_eq!(
            resolved,
            [
                "https://example.com/a/c",
                "https://example.com/d",
                "https://example.com/e?x=1",
                "https://other.org/f",
                "https://example.com/a/g",
                "https://example.com/a/h",
            ]
        );
    }

    #[tokio::test]
    async fn rebase_follows_redirects() {
        let frontier = Arc::new(InMemFrontier::fifo());
        let child = Queue::from_dyn(frontier.clone()).child_of(&page("https://example.com/old/"));
        child
            .clone()
            .rebase(Uri::from_static("https://example.org/new/"));

        child.follow("page").await.unwrap();
        assert_eq!(drain(&frontier).await[0].0, "https://example.org/new/page");
    }

    #[tokio::test]
    async fn skips_unsupported_links() {
        let queue = Queue::new(InMemFrontier::fifo());
        let child = queue.child_of(&page("https://example.com/"));
        for href in ["mailto:a@example.com", "javascript:void(0)", "http://[::1"] {
            let admission = child.follow(href).await.unwrap();
            assert_eq!(admission, Admission::Unsupported, "{href}");
        }
        assert_eq!(
            queue.follow("/relative").await.unwrap(),
            Admission::Unsupported
        );

        let deduped = Queue::new(InMemFrontier::fifo().dedup());
        let child = deduped.child_of(&page("https://example.com/"));
        assert_eq!(
            child
                .follow_all(["/a", "/a", "tel:123", "/b"])
                .await
                .unwrap(),
            2
        );
    }

    #[tokio::test]
    async fn push_reports_admission() {
        let queue = Queue::new(InMemFrontier::fifo().dedup());
        assert!(
            queue
                .push("https://example.com/")
                .await
                .unwrap()
                .is_queued()
        );
        assert_eq!(
            queue.push("https://example.com/").await.unwrap(),
            Admission::Duplicate
        );
        assert!(queue.push("relative").await.is_err());
        assert_eq!(queue.size_hint(), Some(1));
    }
}
