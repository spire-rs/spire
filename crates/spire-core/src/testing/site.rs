use std::collections::{HashMap, HashSet};
use std::future::{Future, ready};
use std::sync::{Arc, Mutex};

use ::http::{HeaderMap, StatusCode, Uri};
use bytes::Bytes;

use super::{MockPage, Visit};
use crate::backend::{Backend, Session, StatusPolicy};
use crate::frontier::seed::Seed;
use crate::http::{Request, Response, ResponseExt};
use crate::identity::Identity;
use crate::sync::lock;
use crate::{Error, Result};

/// Redirects followed at most, as HTTP clients limit them.
const MAX_REDIRECTS: usize = 10;

/// A [`Backend`] serving fixed [`MockPage`]s by URI, for tests.
///
/// URIs without a page get a `404 Not Found`. Statuses are treated as the
/// real backends treat them, by a [`StatusPolicy`]: by default, an error
/// status fails the fetch, so the crawler retries or fails the request.
///
/// Clones share the record of [`requests`](Self::requests). Crawls with
/// identities can be tested too, see [`sent`](Self::sent).
///
/// # Examples
///
/// ```
/// use spire_core::crawler::Crawler;
/// use spire_core::frontier::seed::Seed;
/// use spire_core::testing::{MockPage, MockSite};
/// use spire_core::worker::from_fn;
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> spire_core::Result<()> {
/// let site = MockSite::new()
///     .page("https://example.com/", MockPage::html("<p>Home</p>"))
///     .page("https://example.com/old", MockPage::redirect("https://example.com/"));
///
/// let worker = from_fn(|cx: spire_core::Context<_>| async move { cx.body().await.map(drop) });
/// let crawler = Crawler::new(site.clone(), worker).with_seeds([
///     Seed::get("https://example.com/old")?,
///     Seed::get("https://example.com/missing")?,
/// ]);
///
/// let stats = crawler.run().await?;
/// assert_eq!((stats.succeeded, stats.failed), (1, 1));
/// assert_eq!(site.requests().len(), 2);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Default)]
pub struct MockSite {
    pages: Arc<HashMap<Uri, MockPage>>,
    status_policy: StatusPolicy,
    /// Identities whose proxy refuses every connection.
    unreachable: Arc<HashSet<String>>,
    sent: Arc<Mutex<Vec<Sent>>>,
}

/// A request a [`MockSite`] received, see [`MockSite::sent`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Sent {
    /// The requested URI.
    pub uri: Uri,
    /// The name of the identity it went out as, if any.
    pub identity: Option<String>,
    /// Its headers.
    pub headers: HeaderMap,
}

impl MockSite {
    /// Creates a site without pages.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Serves `page` at `uri`, replacing any previous page there.
    ///
    /// # Panics
    ///
    /// Panics if `uri` is not a valid absolute URI.
    #[must_use]
    pub fn page(mut self, uri: &str, page: MockPage) -> Self {
        Arc::make_mut(&mut self.pages).insert(parse(uri), page);
        self
    }

    /// Replaces the [`StatusPolicy`], [`Classify`](StatusPolicy::Classify)
    /// by default.
    #[must_use]
    pub const fn with_status_policy(mut self, policy: StatusPolicy) -> Self {
        self.status_policy = policy;
        self
    }

    /// Fails every fetch through the identity named `name`, as a proxy
    /// refusing connections would: a retryable error without a status.
    #[must_use]
    pub fn unreachable_through(mut self, name: impl Into<String>) -> Self {
        Arc::make_mut(&mut self.unreachable).insert(name.into());
        self
    }

    /// Returns the URIs fetched so far, in order, including clones' fetches.
    /// Redirects are followed within a fetch, so only the requested URI is
    /// listed.
    #[must_use]
    pub fn requests(&self) -> Vec<Uri> {
        lock(&self.sent)
            .iter()
            .map(|sent| sent.uri.clone())
            .collect()
    }

    /// Returns the requests received so far, in order, with their identity
    /// and headers. Fetches through an
    /// [unreachable](Self::unreachable_through) identity are included.
    #[must_use]
    pub fn sent(&self) -> Vec<Sent> {
        lock(&self.sent).clone()
    }

    /// Prepares a [`Visit`] to `seed`, to run one request through a worker.
    #[must_use]
    pub fn visit(&self, seed: Seed) -> Visit {
        Visit::new(self.clone(), seed)
    }

    /// Serves `uri`, following redirects, and returns the page with the URI
    /// it was served from.
    fn serve(&self, uri: &Uri) -> Result<(MockPage, Uri)> {
        let not_found = || MockPage::status(StatusCode::NOT_FOUND);
        let mut uri = uri.clone();
        for _ in 0..=MAX_REDIRECTS {
            let page = self.pages.get(&uri).cloned().unwrap_or_else(not_found);
            match page.location {
                Some(location) => uri = location,
                None => return Ok((page, uri)),
            }
        }
        Err(Error::new(crate::ErrorKind::Http)
            .with_message("too many redirects")
            .with_uri(uri))
    }
}

/// Whether `request` is conditional and still matches a page with
/// `headers`, as a server answering `304 Not Modified` decides: an
/// `If-None-Match` naming the page's `ETag`, or an `If-Modified-Since` equal
/// to its `Last-Modified`.
fn not_modified(request: &Request, headers: &::http::HeaderMap) -> bool {
    use ::http::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};

    let asked = request.headers();
    if let (Some(etag), Some(none_match)) = (headers.get(ETAG), asked.get(IF_NONE_MATCH)) {
        let none_match = none_match.to_str().unwrap_or_default();
        return none_match
            .split(',')
            .any(|tag| tag.trim() == "*" || tag.trim().as_bytes() == etag.as_bytes());
    }
    matches!(
        (headers.get(LAST_MODIFIED), asked.get(IF_MODIFIED_SINCE)),
        (Some(modified), Some(since)) if modified == since
    )
}

/// Parses an absolute URI given to a [`MockSite`] or [`MockPage`].
pub(super) fn parse(uri: &str) -> Uri {
    match uri.parse::<Uri>() {
        Ok(parsed) if parsed.scheme().is_some() && parsed.authority().is_some() => {
            // Normalised as requests are: an empty path becomes `/`.
            Seed::get(uri).map_or(parsed, |seed| seed.uri().clone())
        }
        _ => panic!("`{uri}` is not an absolute URI"),
    }
}

impl Backend for MockSite {
    type Session = MockSession;

    fn connect(&self) -> impl Future<Output = Result<Self::Session>> + Send {
        ready(Ok(self.session(None)))
    }

    /// Records each request's identity, see [`MockSite::sent`], and refuses
    /// those [unreachable](MockSite::unreachable_through). Proxies are
    /// accepted, as nothing is sent through them.
    fn connect_as(
        &self,
        identity: &Identity,
    ) -> impl Future<Output = Result<Self::Session>> + Send {
        ready(Ok(self.session(Some(identity.clone()))))
    }
}

impl MockSite {
    fn session(&self, identity: Option<Identity>) -> MockSession {
        MockSession {
            site: self.clone(),
            identity,
        }
    }
}

/// The [`Session`] of a [`MockSite`], for one request.
#[derive(Debug)]
pub struct MockSession {
    site: MockSite,
    identity: Option<Identity>,
}

impl MockSession {
    async fn serve(&self, request: &Request) -> Result<Response> {
        let identity = self
            .identity
            .as_ref()
            .map(|identity| identity.name().to_owned());
        lock(&self.site.sent).push(Sent {
            uri: request.uri().clone(),
            identity: identity.clone(),
            headers: request.headers().clone(),
        });
        if identity.is_some_and(|name| self.site.unreachable.contains(&name)) {
            return Err(Error::new(crate::ErrorKind::Http)
                .with_message("the proxy refused the connection")
                .with_uri(request.uri().clone()));
        }
        let (page, served_from) = self.site.serve(request.uri())?;
        if !page.latency.is_zero() {
            tokio::time::sleep(page.latency).await;
        }

        let mut head = Response::new(());
        *head.status_mut() = page.status;
        let unchanged = page.status.is_success() && not_modified(request, &page.headers);
        if unchanged {
            *head.status_mut() = StatusCode::NOT_MODIFIED;
        }
        *head.headers_mut() = page.headers;
        head.set_final_uri(served_from);

        // As the real backends, an error status is rejected with no body.
        let head = self.site.status_policy.check(head)?;
        Ok(head.map(|()| if unchanged { Bytes::new() } else { page.body }))
    }
}

impl Session for MockSession {
    fn fetch(&self, request: &Request) -> impl Future<Output = Result<Response>> + Send {
        self.serve(request)
    }
}
