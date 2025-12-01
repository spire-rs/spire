#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[doc(no_inline)]
pub use async_trait::async_trait;
#[doc(inline)]
pub use routing::Router;
use spire_core::backend::Backend;
pub use spire_core::{Error, ErrorKind, Result, backend, context, dataset};
// Re-export backend implementations when their features are enabled
#[cfg(feature = "reqwest")]
#[cfg_attr(docsrs, doc(cfg(feature = "reqwest")))]
pub use spire_reqwest as reqwest_backend;
#[cfg(feature = "thirtyfour")]
#[cfg_attr(docsrs, doc(cfg(feature = "thirtyfour")))]
pub use spire_thirtyfour as thirtyfour_backend;

pub mod extract;
mod handler;
pub mod middleware;
pub mod routing;

/// Orchestrates the processing of [`Request`]s using provided [`Backend`] and `State`.
///
/// [`Request`]: context::Request
/// [`Backend`]: backend::Backend
pub type Client<B, W = Router<<B as Backend>::Client>> = spire_core::Client<B, W>;

#[doc(hidden)]
pub mod prelude;

#[cfg(test)]
mod tests {
    /// Test that core types can be imported and used
    #[test]
    fn core_types_available() {
        use crate::{Error, ErrorKind, Result};

        let _error = Error::new(ErrorKind::Http, "test error");
        let _result: Result<()> = Ok(());
    }

    /// Test that reqwest backend is available when feature is enabled
    #[test]
    #[cfg(feature = "reqwest")]
    fn reqwest_backend_available() {
        let _backend = crate::reqwest_backend::HttpClient::default();
    }

    /// Test that thirtyfour backend is available when feature is enabled
    #[test]
    #[cfg(feature = "thirtyfour")]
    fn thirtyfour_backend_available() {
        let _backend = crate::thirtyfour_backend::BrowserPool::default();
    }

    /// Test that essential modules are accessible
    #[test]
    fn modules_accessible() {
        // Just test that we can access the module paths
        #[allow(unused_imports)]
        use crate::{extract, middleware, routing};

        // If we can import these, the modules are properly exposed
    }
}
