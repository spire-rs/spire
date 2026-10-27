//! How the crawl connects for a request: as one of its identities, or
//! directly.

use std::sync::Arc;

use tokio::time::Instant;

use crate::Result;
use crate::backend::Backend;
use crate::frontier::seed::Seed;
use crate::http::Request;
use crate::identity::{Health, Identities, Identity, IdentityPool};

/// Connects for the requests of a run, and robots.txt: as one of the run's
/// identities if it has any, or else directly.
pub(crate) struct Connector<B> {
    backend: Arc<B>,
    identities: Option<IdentityPool>,
}

impl<B: Backend> Connector<B> {
    /// Connects every request directly.
    pub(crate) const fn new(backend: Arc<B>) -> Self {
        Self {
            backend,
            identities: None,
        }
    }

    /// Connects every request as one of `identities`.
    pub(crate) fn with_identities(backend: Arc<B>, identities: Identities) -> Self {
        Self {
            backend,
            identities: Some(IdentityPool::new(identities)),
        }
    }

    /// Picks the identity `seed` goes out as, if the run has identities, and
    /// applies it to `request`, made from `seed`.
    ///
    /// # Errors
    ///
    /// Returns an error if no identity can be picked, see
    /// [`IdentityPool::choose`].
    pub(crate) fn identify(&self, seed: &Seed, request: &mut Request) -> Result<Option<Identity>> {
        let Some(pool) = &self.identities else {
            return Ok(None);
        };
        let identity = pool.choose(seed, Instant::now())?;
        identity.apply(request);
        Ok(Some(identity))
    }

    /// Connects a client for a request going out as `identity`, or
    /// directly.
    ///
    /// # Errors
    ///
    /// Returns the backend's error.
    pub(crate) async fn connect(&self, identity: Option<&Identity>) -> Result<B::Session> {
        match identity {
            Some(identity) => self.backend.connect_as(identity).await,
            None => self.backend.connect().await,
        }
    }

    /// Records how a request that went out as `identity` went.
    pub(crate) fn finished(&self, identity: &Identity, health: Health) {
        if let Some(pool) = &self.identities {
            pool.finished(identity, health, Instant::now());
        }
    }
}
