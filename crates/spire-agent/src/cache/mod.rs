//! Optional answer caches. Keys are hashes of content, instructions, schema,
//! token limit and an explicit provider/model namespace, not just page URLs.

mod memory;
#[cfg(feature = "redb")]
#[cfg_attr(docsrs, doc(cfg(feature = "redb")))]
mod persistent;

use futures::future::BoxFuture;
pub use memory::InMemCache;
#[cfg(feature = "redb")]
#[cfg_attr(docsrs, doc(cfg(feature = "redb")))]
pub use persistent::RedbCache;

use crate::Result;

/// Stores validated answer text. Cache hits do not spend model budgets.
pub trait Cache: Send + Sync + 'static {
    /// Reads an answer.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be read.
    fn get<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<Option<String>>>;

    /// Stores or replaces an answer.
    ///
    /// # Errors
    ///
    /// Returns an error if storage cannot be written.
    fn put<'a>(&'a self, key: &'a str, answer: &'a str) -> BoxFuture<'a, Result<()>>;
}

/// Adds model answer caching to a crawl store without another database.
/// Each store selects its own [`Cache`] implementation.
pub trait AgentStoreExt {
    /// The answer cache backed by this store.
    type Cache: Cache;

    /// Returns the model answer cache sharing this store.
    fn agent_cache(&self) -> Self::Cache;
}
