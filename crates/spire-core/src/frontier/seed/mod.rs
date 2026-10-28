//! Seeds: [`Seed`], a request waiting in the frontier, and its
//! [`Fingerprint`].

use std::sync::LazyLock;

use ::http::header::IntoHeaderName;
use ::http::{HeaderMap, HeaderValue, Method, Uri};
use bytes::Bytes;

mod fingerprint;
#[cfg(feature = "serde")]
mod repr;

pub use fingerprint::Fingerprint;

use crate::crawler::{Attempt, Depth, Tag};
use crate::http::{Request, RequestMeta, UriExt};
use crate::{Error, Result};

/// A request waiting in the frontier.
///
/// Unlike a [`Request`], a seed can be cloned for retries, fingerprinted for
/// deduplication and stored by persistent frontiers. The crawler turns it
/// into a [`Request`] right before fetching.
#[derive(Debug, Clone)]
pub struct Seed {
    method: Method,
    uri: Uri,
    /// Headers and body, which most seeds do not have: boxed, so a seed
    /// stays small as it moves through the frontier.
    extras: Option<Box<Extras>>,
    tag: Option<Tag>,
    depth: Depth,
    priority: i32,
    attempt: Attempt,
}

impl Seed {
    /// Creates a seed for `method` and an absolute `uri`.
    ///
    /// # Errors
    ///
    /// Returns an [`Invalid`](crate::ErrorKind::Invalid) error if `uri` has
    /// no scheme or authority.
    pub fn new(method: Method, uri: Uri) -> Result<Self> {
        if uri.scheme().is_none() || uri.authority().is_none() {
            return Err(Error::invalid(format!("seed uri `{uri}` is not absolute")));
        }

        Ok(Self {
            method,
            uri,
            extras: None,
            tag: None,
            depth: Depth::ROOT,
            priority: 0,
            attempt: Attempt::FIRST,
        })
    }

    /// Creates a `GET` seed.
    ///
    /// # Errors
    ///
    /// Returns an [`Invalid`](crate::ErrorKind::Invalid) error if `uri` is
    /// malformed or not absolute.
    pub fn get<U>(uri: U) -> Result<Self>
    where
        U: TryInto<Uri>,
        U::Error: Into<Error>,
    {
        Self::new(Method::GET, uri.try_into().map_err(Into::into)?)
    }

    /// Sets the tag that routes this seed to its handler.
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<Tag>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Sets the depth.
    #[must_use]
    pub const fn with_depth(mut self, depth: Depth) -> Self {
        self.depth = depth;
        self
    }

    /// Sets the priority, `0` by default; frontiers that order by priority,
    /// such as [`InMemFrontier::priority`](crate::frontier::InMemFrontier::priority),
    /// crawl higher priorities first.
    #[must_use]
    pub const fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Inserts a header, replacing any previous value.
    #[must_use]
    pub fn with_header(mut self, name: impl IntoHeaderName, value: HeaderValue) -> Self {
        self.extras_mut().headers.insert(name, value);
        self
    }

    /// Sets the body.
    #[must_use]
    pub fn with_body(mut self, body: impl Into<Bytes>) -> Self {
        self.extras_mut().body = body.into();
        self
    }

    /// Sends the request as the identity named `name`, e.g. a logged-in
    /// account, rather than one the crawl picks; see
    /// [`Crawler::with_identities`](crate::crawler::Crawler::with_identities).
    #[must_use]
    pub fn with_identity(mut self, name: impl Into<Box<str>>) -> Self {
        self.extras_mut().identity = Some(name.into());
        self
    }

    /// Returns the headers, body and identity, creating them on first use.
    fn extras_mut(&mut self) -> &mut Extras {
        self.extras.get_or_insert_with(Box::default)
    }

    /// Returns a copy of this seed for its next attempt.
    #[must_use]
    pub(crate) fn next_attempt(mut self) -> Self {
        self.attempt = self.attempt.next();
        self
    }

    /// Returns this seed as on its first attempt, e.g. to crawl it again.
    #[must_use]
    pub(crate) const fn first_attempt(mut self) -> Self {
        self.attempt = Attempt::FIRST;
        self
    }

    /// Returns the method.
    #[must_use]
    pub const fn method(&self) -> &Method {
        &self.method
    }

    /// Returns the URI.
    #[must_use]
    pub const fn uri(&self) -> &Uri {
        &self.uri
    }

    /// Returns the headers.
    #[must_use]
    pub fn headers(&self) -> &HeaderMap {
        static EMPTY: LazyLock<HeaderMap> = LazyLock::new(HeaderMap::new);
        self.extras
            .as_ref()
            .map_or(&EMPTY, |extras| &extras.headers)
    }

    /// Returns the body.
    #[must_use]
    pub fn body(&self) -> &Bytes {
        static EMPTY: Bytes = Bytes::new();
        self.extras.as_ref().map_or(&EMPTY, |extras| &extras.body)
    }

    /// Returns the tag, if any.
    #[must_use]
    pub const fn tag(&self) -> Option<&Tag> {
        self.tag.as_ref()
    }

    /// Returns the depth.
    #[must_use]
    pub const fn depth(&self) -> Depth {
        self.depth
    }

    /// Returns the name of the identity the seed is pinned to, if any.
    #[must_use]
    pub fn identity(&self) -> Option<&str> {
        self.extras.as_ref()?.identity.as_deref()
    }

    /// Returns the priority.
    #[must_use]
    pub const fn priority(&self) -> i32 {
        self.priority
    }

    /// Returns the attempt.
    #[must_use]
    pub const fn attempt(&self) -> Attempt {
        self.attempt
    }

    /// Returns the canonical form of the URI, which URIs naming the same
    /// resource share; see [`fingerprint`](Self::fingerprint).
    #[must_use]
    pub fn canonical_uri(&self) -> String {
        self.uri.canonical()
    }

    /// Returns a fingerprint that identifies the request regardless of its
    /// tag, depth, priority and attempt.
    ///
    /// It hashes the method, the [canonical URI](Self::canonical_uri) and the
    /// body, so `HTTPS://Example.com:443/a?b=2&a=1&utm_source=x` and
    /// `https://example.com/a?a=1&b=2` share a fingerprint. Headers are not
    /// part of it.
    #[must_use]
    pub fn fingerprint(&self) -> Fingerprint {
        fingerprint(&self.method, &self.uri, self.body())
    }

    /// Converts the seed into a [`Request`] carrying its tag, depth and
    /// attempt as extensions.
    #[must_use]
    pub fn into_request(self) -> Request {
        let Extras { headers, body, .. } = self.extras.map(|extras| *extras).unwrap_or_default();
        let mut request = Request::new(body);
        *request.method_mut() = self.method;
        *request.uri_mut() = self.uri;
        *request.headers_mut() = headers;

        request.extensions_mut().insert(RequestMeta {
            tag: self.tag,
            depth: self.depth,
            attempt: self.attempt,
            identity: None,
        });
        request
    }
}

/// The fingerprint of a request with `method`, `uri` and `body`, as
/// [`Seed::fingerprint`] computes it, e.g. for a request built from a seed.
pub(crate) fn fingerprint(method: &Method, uri: &Uri, body: &[u8]) -> Fingerprint {
    Fingerprint::of(&[method.as_str().as_bytes(), uri.canonical().as_bytes(), body])
}

/// The rarely set parts of a [`Seed`].
#[derive(Debug, Clone, Default)]
struct Extras {
    headers: HeaderMap,
    body: Bytes,
    identity: Option<Box<str>>,
}

impl TryFrom<&str> for Seed {
    type Error = Error;

    fn try_from(uri: &str) -> Result<Self> {
        Self::get(uri)
    }
}

impl TryFrom<String> for Seed {
    type Error = Error;

    fn try_from(uri: String) -> Result<Self> {
        Self::get(uri)
    }
}

impl TryFrom<Uri> for Seed {
    type Error = Error;

    fn try_from(uri: Uri) -> Result<Self> {
        Self::new(Method::GET, uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::RequestExt;

    fn fingerprint(uri: &str) -> Fingerprint {
        Seed::get(uri).unwrap().fingerprint()
    }

    #[test]
    fn rejects_relative_uris() {
        assert!(Seed::get("/relative").is_err());
        assert!(Seed::get("not a uri").is_err());
        assert!(Seed::get("https://example.com").is_ok());
    }

    #[test]
    fn fingerprint_normalizes_scheme_host_and_port() {
        let base = fingerprint("https://example.com/a?b=1");
        assert_eq!(base, fingerprint("HTTPS://Example.COM:443/a?b=1"));
        assert_ne!(base, fingerprint("https://example.com:8443/a?b=1"));
        assert_ne!(base, fingerprint("https://example.com/a?b=2"));
        assert_eq!(
            base,
            fingerprint("https://example.com/a?utm_source=x&b=1#top")
        );
        assert_eq!(
            fingerprint("https://example.com"),
            fingerprint("https://example.com/")
        );
    }

    #[test]
    fn fingerprint_ignores_crawl_metadata() {
        let seed = Seed::get("https://example.com/").unwrap();
        let tagged = seed.clone().with_tag("page").with_depth(Depth::new(3));
        assert_eq!(seed.fingerprint(), tagged.fingerprint());

        let post = Seed::new(Method::POST, seed.uri().clone()).unwrap();
        assert_ne!(seed.fingerprint(), post.fingerprint());
        assert_ne!(
            post.fingerprint(),
            post.clone().with_body("x").fingerprint()
        );
    }

    #[test]
    fn into_request_carries_metadata() {
        let seed = Seed::get("https://example.com/")
            .unwrap()
            .with_tag("page")
            .with_depth(Depth::new(2))
            .next_attempt();
        let request = seed.into_request();
        assert_eq!(request.uri(), "https://example.com/");
        assert_eq!(request.tag(), Some(&Tag::from("page")));
        assert_eq!(request.depth(), Depth::new(2));
        assert_eq!(request.attempt(), Attempt::new(1));
    }
}
