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
mod weak;

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use http::header::IntoHeaderName;
use http::{HeaderMap, HeaderValue, Uri};
#[cfg(feature = "user-agents")]
#[cfg_attr(docsrs, doc(cfg(feature = "user-agents")))]
pub use uagent::{Browser, Platform, Profile, UserAgents};

pub use self::identities::Identities;
pub(crate) use self::pool::{Health, IdentityPool};
pub use self::weak::WeakIdentity;
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
    #[cfg(feature = "user-agents")]
    profile: Option<Profile>,
}

impl Identity {
    /// Creates an identity named `name`, with no proxy and no headers.
    #[must_use]
    pub fn new(name: impl Into<Box<str>>) -> Self {
        Self(Arc::new(Inner {
            name: name.into(),
            proxy: None,
            headers: HeaderMap::new(),
            #[cfg(feature = "user-agents")]
            profile: None,
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

    /// Presents as this browser profile. Identity headers override the
    /// profile's headers, and request headers override both.
    ///
    /// Real browser backends refuse profiles: their own browser supplies
    /// its user agent and client hints.
    #[cfg(feature = "user-agents")]
    #[cfg_attr(docsrs, doc(cfg(feature = "user-agents")))]
    #[must_use]
    pub fn with_profile(mut self, profile: Profile) -> Self {
        Arc::make_mut(&mut self.0).profile = Some(profile);
        self
    }

    /// Returns the browser profile, kept separate from identity headers.
    #[cfg(feature = "user-agents")]
    #[cfg_attr(docsrs, doc(cfg(feature = "user-agents")))]
    #[must_use]
    pub fn profile(&self) -> Option<&Profile> {
        self.0.profile.as_ref()
    }

    /// Fails if this identity has a browser profile, for backends whose
    /// real browser supplies its own user agent and client hints.
    ///
    /// # Errors
    ///
    /// Returns an [`Invalid`](crate::ErrorKind::Invalid) error if a profile
    /// is set. Without `user-agents`, always succeeds.
    pub fn require_no_profile(&self) -> Result<()> {
        #[cfg(feature = "user-agents")]
        if self.profile().is_some() {
            return Err(Error::invalid(format!(
                "identity `{}` has a user-agent profile, which a real browser cannot use",
                self.name()
            )));
        }
        Ok(())
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

    /// Returns the identity headers, excluding its browser profile.
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
        #[cfg(feature = "user-agents")]
        if let Some(profile) = self.profile() {
            for header in profile.headers() {
                if !headers.contains_key(header.name) {
                    // uagent generates validated names and values from its captures.
                    let name = http::HeaderName::from_static(header.name);
                    let value = HeaderValue::from_str(&header.value)
                        .expect("uagent profile headers are valid HTTP headers");
                    headers.insert(name, value);
                }
            }
        }
        let meta = request
            .extensions_mut()
            .get_or_insert_default::<RequestMeta>();
        meta.identity = Some(self.clone());
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

    #[cfg(feature = "user-agents")]
    #[test]
    fn replacing_a_profile_preserves_explicit_headers_and_clones() {
        let chrome = Profile::new(Browser::Chrome, Platform::Windows).unwrap();
        let firefox = Profile::new(Browser::Firefox, Platform::Linux).unwrap();
        let original = Identity::new("a")
            .with_profile(chrome.clone())
            .with_header(USER_AGENT, HeaderValue::from_static("explicit"));
        let changed = original.clone().with_profile(firefox.clone());
        assert_eq!(original.profile(), Some(&chrome));
        assert_eq!(changed.profile(), Some(&firefox));
        assert_eq!(changed.headers()[USER_AGENT], "explicit");
        assert_eq!(changed.headers().len(), 1);
        assert!(changed.require_no_profile().is_err());
        assert!(Identity::new("plain").require_no_profile().is_ok());
    }

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
