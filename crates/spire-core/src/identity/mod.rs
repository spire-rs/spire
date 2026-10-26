//! Identities: who a request appears to come from.
//!
//! A request reveals its sender in several places at once: its IP, through
//! a proxy; its user agent and the headers that go with it; and its cookies.
//! An [`Identity`] keeps them together, one visitor, so they always agree:
//! one IP does not send many user agents, and a session cookie set through
//! one proxy is not sent through another.
//!
//! [`Identities`] are defined once, on the
//! [`Crawler`](crate::Crawler::with_identities), which picks one for every
//! request, applies its headers, and connects as it, see
//! [`Backend::connect_as`](crate::backend::Backend::connect_as). Identities that
//! keep failing are set aside, or replaced by new ones.
//!
//! ```
//! use spire_core::Result;
//! use spire_core::http::HeaderValue;
//! use spire_core::http::header::USER_AGENT;
//! use spire_core::identity::{Identities, Identity};
//!
//! # fn main() -> Result<()> {
//! // A rotating gateway: each session id is a different exit IP.
//! let identities = Identities::from_fn(|n| {
//!     Ok(Identity::new(format!("session-{n}"))
//!         .with_proxy(format!("http://user-session-{n}:secret@gate.example:7000"))?
//!         .with_header(USER_AGENT, HeaderValue::from_static("MyBot/1.0")))
//! })
//! .max_active(100)
//! .sticky_per_host();
//! # Ok(())
//! # }
//! ```

mod identities;
mod pool;

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Weak};

use http::header::IntoHeaderName;
use http::{HeaderMap, HeaderValue, Uri};

pub use self::identities::Identities;
pub(crate) use self::pool::{Health, IdentityPool};
use crate::request::{Request, RequestMeta};
use crate::{Error, Result};

/// Who a request appears to come from: a name, an optional proxy, and the
/// headers it sends.
///
/// Identities are compared and hashed by name. Cloning is cheap.
#[derive(Clone)]
pub struct Identity(Arc<Inner>);

#[derive(Clone)]
struct Inner {
    name: Box<str>,
    proxy: Option<Uri>,
    headers: HeaderMap,
}

impl Identity {
    /// Creates an identity named `name`, with no proxy and no headers.
    #[must_use]
    pub fn new(name: impl Into<Box<str>>) -> Self {
        Self(Arc::new(Inner {
            name: name.into(),
            proxy: None,
            headers: HeaderMap::new(),
        }))
    }

    /// Fails if requests should go through a proxy: for a backend that
    /// cannot route through one, so it never sends them from the crawler's
    /// own IP instead.
    ///
    /// # Errors
    ///
    /// Returns an [`Invalid`](crate::ErrorKind::Invalid) error if the
    /// identity has a proxy.
    pub fn require_no_proxy(&self) -> Result<()> {
        match self.proxy() {
            Some(_) => Err(Error::invalid(format!(
                "identity `{}` has a proxy, which this backend cannot send through",
                self.name()
            ))),
            None => Ok(()),
        }
    }

    /// Sends requests through `proxy`, an `http`, `https` or `socks5` URL,
    /// with credentials if it needs them.
    ///
    /// # Errors
    ///
    /// Returns an [`Invalid`](crate::ErrorKind::Invalid) error if `proxy`
    /// is malformed or not absolute.
    pub fn with_proxy<U>(mut self, proxy: U) -> Result<Self>
    where
        U: TryInto<Uri>,
        U::Error: Into<Error>,
    {
        let proxy: Uri = proxy.try_into().map_err(Into::into)?;
        if proxy.scheme().is_none() || proxy.authority().is_none() {
            let message = format!("proxy of identity `{}` is not absolute", self.name());
            return Err(Error::invalid(message));
        }
        Arc::make_mut(&mut self.0).proxy = Some(proxy);
        Ok(self)
    }

    /// Sends `name` with `value` on every request, unless the request sets
    /// it itself.
    #[must_use]
    pub fn with_header(mut self, name: impl IntoHeaderName, value: HeaderValue) -> Self {
        Arc::make_mut(&mut self.0).headers.insert(name, value);
        self
    }

    /// Returns the name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0.name
    }

    /// Returns the proxy, if requests go through one.
    #[must_use]
    pub fn proxy(&self) -> Option<&Uri> {
        self.0.proxy.as_ref()
    }

    /// Returns the headers sent on every request.
    #[must_use]
    pub fn headers(&self) -> &HeaderMap {
        &self.0.headers
    }

    /// Returns a weak reference to this identity, which tells whether it is
    /// still in use: once the crawl retires it and its last request ends,
    /// it is gone. Backends use it to drop what they keep per identity,
    /// such as a client or a browser profile.
    #[must_use]
    pub fn downgrade(&self) -> WeakIdentity {
        WeakIdentity(Arc::downgrade(&self.0))
    }

    /// Sends `request` as this identity: with its headers, unless the
    /// request sets them, and recorded for
    /// [`RequestExt::identity`](crate::request::RequestExt::identity).
    pub(crate) fn apply(&self, request: &mut Request) {
        let headers = request.headers_mut();
        for (name, value) in self.headers() {
            if !headers.contains_key(name) {
                headers.insert(name.clone(), value.clone());
            }
        }
        let meta = request
            .extensions_mut()
            .get_or_insert_default::<RequestMeta>();
        meta.identity = Some(self.clone());
    }
}

/// A weak reference to an [`Identity`], from [`Identity::downgrade`].
#[derive(Clone)]
pub struct WeakIdentity(Weak<Inner>);

impl WeakIdentity {
    /// Returns the identity, unless it is gone.
    #[must_use]
    pub fn upgrade(&self) -> Option<Identity> {
        self.0.upgrade().map(Identity)
    }

    /// Whether the identity is still in use.
    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.0.strong_count() > 0
    }

    /// Whether this refers to `identity` itself, not merely one with the
    /// same name.
    #[must_use]
    pub fn refers_to(&self, identity: &Identity) -> bool {
        std::ptr::eq(self.0.as_ptr(), Arc::as_ptr(&identity.0))
    }
}

impl fmt::Debug for WeakIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("WeakIdentity")
            .field(&self.upgrade().as_ref().map(Identity::name))
            .finish()
    }
}

/// Shows the proxy without its credentials.
impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let proxy = self.proxy().map(|proxy| {
            let host = proxy.host().unwrap_or_default();
            match proxy.port_u16() {
                Some(port) => format!("{host}:{port}"),
                None => host.to_owned(),
            }
        });
        f.debug_struct("Identity")
            .field("name", &self.name())
            .field("proxy", &proxy)
            .field("headers", &self.headers().keys().collect::<Vec<_>>())
            .finish()
    }
}

impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}

impl Eq for Identity {}

impl Hash for Identity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use http::header::USER_AGENT;

    use super::*;

    #[test]
    fn debug_hides_proxy_credentials() {
        let identity = Identity::new("a")
            // Fake credentials, for the test.
            .with_proxy("http://user:secret@proxy.test:8080") // trufflehog:ignore
            .unwrap()
            .with_header(USER_AGENT, HeaderValue::from_static("Bot"));
        let debug = format!("{identity:?}");
        assert!(debug.contains("proxy.test:8080"), "{debug}");
        assert!(!debug.contains("secret"), "{debug}");
        assert_eq!(identity.proxy().unwrap().host(), Some("proxy.test"));
    }

    #[test]
    fn weak_references_outlive_their_identity() {
        let identity = Identity::new("a");
        let weak = identity.downgrade();
        assert!(weak.is_alive() && weak.refers_to(&identity));
        assert!(!weak.refers_to(&Identity::new("a")));
        drop(identity);
        assert!(!weak.is_alive());
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn only_proxied_identities_require_a_proxy() {
        assert!(Identity::new("a").require_no_proxy().is_ok());
        let proxied = Identity::new("a")
            .with_proxy("http://proxy.test:8080")
            .unwrap();
        assert!(proxied.require_no_proxy().is_err());
    }

    #[test]
    fn rejects_relative_proxies() {
        assert!(Identity::new("a").with_proxy("/proxy").is_err());
        assert!(Identity::new("a").with_proxy("not a url").is_err());
    }
}
