use std::collections::HashMap;

use deadpool::managed::{Manager, Metrics, RecycleResult};

use spire_core::{Error, Result};

use crate::connect::BrowserConnection;

/// Pool manager for browser connections.
///
/// This type implements the `deadpool::managed::Manager` trait to handle
/// creation and recycling of browser instances in the pool.
///
/// ## Note
///
/// The current implementation is a stub. Full browser lifecycle management
/// (creation, health checks, recycling) will be implemented in a future version.
#[must_use]
pub struct BrowserManager {
    connections: HashMap<u64, BrowserConnection>,
}

impl BrowserManager {
    /// Creates a new [`BrowserManager`].
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a browser connection to the manager.
    ///
    /// ## Note
    ///
    /// This functionality is not yet implemented and will panic.
    pub fn with(&self, _conn: BrowserConnection) -> Result<()> {
        todo!("BrowserManager::with is not yet implemented")
    }

    /// Automatically scales the browser pool based on demand.
    ///
    /// ## Note
    ///
    /// This functionality is not yet implemented and will panic.
    fn autoscale(&self) -> Result<()> {
        todo!("BrowserManager::autoscale is not yet implemented")
    }
}

impl Default for BrowserManager {
    fn default() -> Self {
        // TODO: BLOCKED: https://github.com/jonhoo/fantoccini/pull/245
        // Full implementation pending upstream changes

        Self {
            connections: HashMap::new(),
        }
    }
}

impl Manager for BrowserManager {
    type Type = BrowserConnection;
    type Error = Error;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        todo!("BrowserManager::create is not yet implemented")
    }

    async fn recycle(
        &self,
        _obj: &mut Self::Type,
        _metrics: &Metrics,
    ) -> RecycleResult<Self::Error> {
        todo!("BrowserManager::recycle is not yet implemented")
    }
}
