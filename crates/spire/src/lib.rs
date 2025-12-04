#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[doc(inline)]
pub use routing::Router;
#[doc(no_inline)]
pub use spire_core::async_trait;
use spire_core::backend::Backend;
pub use spire_core::{Error, ErrorKind, Result, backend, context, dataset, http};
#[cfg(feature = "reqwest")]
#[cfg_attr(docsrs, doc(cfg(feature = "reqwest")))]
pub use spire_reqwest::HttpClient;
#[cfg(feature = "thirtyfour")]
#[cfg_attr(docsrs, doc(cfg(feature = "thirtyfour")))]
pub use spire_thirtyfour::{
    BrowserBackend, BrowserConnection, BrowserError, BrowserPool, BrowserType, ClientConfig,
    NavigationErrorType, PoolConfig, PoolConfigBuilder, WebDriverConfig, WebDriverConfigBuilder,
};

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
        let _backend = crate::HttpClient::default();
    }

    /// Test that thirtyfour backend is available when feature is enabled
    #[test]
    #[cfg(feature = "thirtyfour")]
    fn thirtyfour_backend_available() {
        let _backend = crate::BrowserBackend::builder()
            .with_unmanaged("http://127.0.0.1:4444")
            .build()
            .expect("Failed to build browser pool");
    }
}
