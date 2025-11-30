#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
//!
//! # Feature Flags
//!
//! Spire uses feature flags to enable optional functionality:
//!
//! ## Backend Implementations
//!
//! - **`reqwest`** - Enables the reqwest-based HTTP client backend
//! - **`fantoccini`** - Enables the WebDriver/browser automation backend
//!
//! ## Additional Features
//!
//! - **`macros`** - Enables procedural macros for deriving extractors
//! - **`tracing`** - Enables tracing/logging support
//! - **`trace`** - Enables detailed trace-level instrumentation
//! - **`metric`** - Enables metrics collection
//! - **`full`** - Enables all features (macros, tracing, reqwest, fantoccini)
//!

//!
//! # Examples
//!
//! ## Using with Reqwest Backend
//!
//! ```toml
//! [dependencies]
//! spire = { version = "0.1", features = ["reqwest"] }
//! ```
//!
//! ## Using with Fantoccini Backend
//!
//! ```toml
//! [dependencies]
//! spire = { version = "0.1", features = ["fantoccini"] }
//! ```
//!
//! ## Using All Features
//!
//! ```toml
//! [dependencies]
//! spire = { version = "0.1", features = ["full"] }
//! ```
// #![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

#[doc(no_inline)]
pub use async_trait::async_trait;

#[doc(inline)]
pub use routing::Router;
use spire_core::backend::Backend;
pub use spire_core::{backend, context, dataset};
pub use spire_core::{Error, ErrorKind, Result};

// Re-export backend implementations when their features are enabled
#[cfg(feature = "reqwest")]
#[cfg_attr(docsrs, doc(cfg(feature = "reqwest")))]
pub use spire_reqwest as reqwest_backend;

#[cfg(feature = "fantoccini")]
#[cfg_attr(docsrs, doc(cfg(feature = "fantoccini")))]
pub use spire_fantoccini as fantoccini_backend;

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
pub mod prelude {}

#[cfg(test)]
mod test {
    use futures::SinkExt;

    use spire_core::dataset::future::DataSink;

    use crate::context::RequestQueue;
    use crate::dataset::InMemDataset;
    use crate::{Client, Result, Router};

    #[test]
    #[cfg(feature = "reqwest")]
    fn with_reqwest_client() {
        async fn handler(queue: RequestQueue, mut dataset: DataSink<u64>) -> Result<()> {
            dataset.feed(1).await?;
            Ok(())
        }

        let router = Router::new()
            .route("main", handler)
            .route("page", handler)
            .fallback(handler);

        let backend = crate::reqwest_backend::HttpClient::default();
        let client = Client::new(backend, router)
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new());

        let _ = client.run();
    }

    #[test]
    #[cfg(feature = "fantoccini")]
    fn with_fantoccini_driver() {
        async fn handler(queue: RequestQueue, mut dataset: DataSink<u64>) -> Result<()> {
            dataset.feed(1).await?;
            Ok(())
        }

        let router = Router::new()
            .route("main", handler)
            .route("page", handler)
            .fallback(handler);

        let backend = crate::fantoccini_backend::BrowserPool::builder().build();
        let client = Client::new(backend, router)
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new());

        let _ = client.run();
    }
}
