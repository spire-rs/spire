//! The reqwest clients of an [`ReqwestBackend`](crate::ReqwestBackend): one for
//! requests without an identity, and one per identity, with the identity's
//! proxy and its own cookies.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use spire_core::identity::{Identity, WeakIdentity};
use spire_core::{Error, Result};

/// Makes the builder every client starts from.
pub(crate) type Builder = dyn Fn() -> reqwest::ClientBuilder + Send + Sync;

/// Every reqwest client of an [`ReqwestBackend`](crate::ReqwestBackend), all from
/// one configuration.
#[derive(Clone)]
pub(crate) struct Clients {
    builder: Arc<Builder>,
    /// For requests without an identity.
    direct: reqwest::Client,
    /// One per identity, kept while the identity is in use.
    identities: Arc<Mutex<HashMap<Box<str>, Built>>>,
}

/// The client of an identity.
struct Built {
    identity: WeakIdentity,
    client: reqwest::Client,
}

/// reqwest's defaults.
impl Default for Clients {
    fn default() -> Self {
        Self {
            builder: Arc::new(reqwest::Client::builder),
            direct: reqwest::Client::default(),
            identities: Arc::default(),
        }
    }
}

impl Clients {
    /// Builds the direct client from `builder`'s configuration, which
    /// identities' clients are built from too.
    ///
    /// # Errors
    ///
    /// Returns a [`Backend`](spire_core::ErrorKind::Backend) error if
    /// reqwest cannot build it, e.g. when TLS fails to initialize.
    pub(crate) fn new(builder: Arc<Builder>) -> Result<Self> {
        let direct = builder().build().map_err(|error| {
            Error::backend("failed to build the HTTP client").with_source(error)
        })?;
        Ok(Self {
            builder,
            direct,
            identities: Arc::default(),
        })
    }

    /// Returns the client for requests without an identity.
    pub(crate) const fn direct(&self) -> &reqwest::Client {
        &self.direct
    }

    /// Returns the client of `identity`, building it on first use.
    ///
    /// # Errors
    ///
    /// Returns an [`Invalid`](spire_core::ErrorKind::Invalid) error if
    /// reqwest does not support the identity's proxy, e.g. `socks5` without
    /// the `socks` feature, or a [`Backend`](spire_core::ErrorKind::Backend)
    /// error if the client cannot be built.
    pub(crate) fn of(&self, identity: &Identity) -> Result<reqwest::Client> {
        let mut identities = self
            .identities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(built) = identities.get(identity.name())
            && built.identity.refers_to(identity)
        {
            return Ok(built.client.clone());
        }

        let client = self.build_for(identity)?;
        // Identities the crawl retired are gone once their last request
        // ended, and their clients with them.
        identities.retain(|_, built| built.identity.is_alive());
        let built = Built {
            identity: identity.downgrade(),
            client: client.clone(),
        };
        identities.insert(identity.name().into(), built);
        Ok(client)
    }

    fn build_for(&self, identity: &Identity) -> Result<reqwest::Client> {
        let mut builder = (self.builder)();
        if let Some(proxy) = identity.proxy() {
            // reqwest takes a SOCKS URL without its `socks` feature, and
            // only fails once connecting, as if the proxy were down.
            let socks = matches!(proxy.scheme_str(), Some("socks5" | "socks5h"));
            if socks && !cfg!(feature = "socks") {
                let message = format!(
                    "identity `{}` has a SOCKS proxy, which needs the `socks` feature",
                    identity.name()
                );
                return Err(Error::invalid(message));
            }
            // A proxy of its own also turns off proxies from the system.
            let proxy = reqwest::Proxy::all(proxy.to_string()).map_err(|error| {
                let message = format!("unsupported proxy for identity `{}`", identity.name());
                Error::invalid(message).with_source(error)
            })?;
            builder = builder.proxy(proxy);
        }
        // A jar of its own, replacing any the builder shares, so cookies
        // set for one identity are never sent by another.
        #[cfg(feature = "cookies")]
        let builder = builder.cookie_store(true);
        builder.build().map_err(|error| {
            let message = format!(
                "failed to build the client of identity `{}`",
                identity.name()
            );
            Error::backend(message).with_source(error)
        })
    }

    /// Returns how many identities have a client, for tests.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.identities
            .lock()
            .map_or(0, |identities| identities.len())
    }
}

impl fmt::Debug for Clients {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let identities = self
            .identities
            .lock()
            .map_or(0, |identities| identities.len());
        f.debug_struct("Clients")
            .field("identities", &identities)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clients() -> Clients {
        Clients::default()
    }

    #[test]
    fn clients_are_kept_while_their_identity_is_in_use() {
        let clients = clients();
        let a = Identity::new("a");
        clients.of(&a).unwrap();
        clients.of(&a).unwrap();
        assert_eq!(clients.len(), 1);

        // Gone with its identity, once another client is built.
        drop(a);
        clients.of(&Identity::new("b")).unwrap();
        assert_eq!(clients.len(), 1);
    }

    #[test]
    fn a_new_identity_with_an_old_name_gets_a_new_client() {
        let clients = clients();
        let old = Identity::new("a");
        clients.of(&old).unwrap();
        let new = Identity::new("a")
            .with_proxy("http://127.0.0.1:3128")
            .unwrap();
        clients.of(&new).unwrap();
        assert_eq!(clients.len(), 1);
    }
}
