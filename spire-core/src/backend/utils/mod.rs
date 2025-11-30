//! Utility middlewares for [`Backend`], [`Client`], and [`Worker`].
//!
//! This module provides composable middleware layers that add functionality like
//! tracing, metrics collection, and debugging capabilities to backend components.
//!
//! # Available Middlewares
//!
//! ## Debugging
//!
//! - [`Noop`] - No-op implementation for all traits, useful for testing and prototyping
//!
//! ## Observability
//!
//! - [`Trace`] - Adds distributed tracing instrumentation (requires `trace` feature)
//! - [`Metric`] - Collects performance metrics (requires `metric` feature)
//!
//! # Examples
//!
//! ## Using Noop for Testing
//!
//! ```ignore
//! use spire_core::backend::utils::Noop;
//! use spire_core::Client;
//!
//! let backend = Noop::default();
//! let worker = Noop::default();
//! let client = Client::new(backend, worker);
//! ```
//!
//! ## Adding Tracing
//!
//! ```ignore
//! use spire_core::backend::utils::Trace;
//!
//! let backend = Trace::new(my_backend);
//! let worker = Trace::new(my_worker);
//! ```
//!
//! [`Backend`]: crate::backend::Backend
//! [`Client`]: crate::backend::Client
//! [`Worker`]: crate::backend::Worker

mod debug;
#[cfg(feature = "metric")]
#[cfg_attr(docsrs, doc(cfg(feature = "metric")))]
mod metric;
#[cfg(feature = "trace")]
#[cfg_attr(docsrs, doc(cfg(feature = "trace")))]
mod trace;

pub use debug::Noop;
#[cfg(feature = "metric")]
#[cfg_attr(docsrs, doc(cfg(feature = "metric")))]
pub use metric::{Metric, MetricLayer};
#[cfg(feature = "trace")]
#[cfg_attr(docsrs, doc(cfg(feature = "trace")))]
pub use trace::{Trace, TraceLayer};

pub mod futures {
    //! Future types for [`spire-core`] middlewares.
    //!
    //! [`spire-core`]: crate

    #[cfg(feature = "metric")]
    #[cfg_attr(docsrs, doc(cfg(feature = "metric")))]
    pub use crate::backend::utils::metric::MetricFuture;
    #[cfg(feature = "trace")]
    #[cfg_attr(docsrs, doc(cfg(feature = "trace")))]
    pub use crate::backend::utils::trace::TraceFuture;
}

#[cfg(test)]
mod test {
    use http::Request;

    use crate::backend::utils::Noop;
    use crate::dataset::InMemDataset;
    use crate::{Client, Result};

    #[tokio::test]
    async fn noop() -> Result<()> {
        let entity = Noop::default();
        let request = Request::get("https://example.com/").body(());
        let client = Client::new(entity.clone(), entity)
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new())
            .with_initial_request(request.unwrap());

        let _ = client.dataset::<u64>();
        let _ = client.run().await?;
        Ok(())
    }

    #[tokio::test]
    #[cfg(feature = "trace")]
    #[tracing_test::traced_test]
    async fn noop_trace() -> Result<()> {
        use crate::backend::utils::Trace;

        let entity = Trace::new(Noop::default());
        let request = Request::get("https://example.com/").body(());
        let client = Client::new(entity.clone(), entity)
            .with_request_queue(InMemDataset::stack())
            .with_dataset(InMemDataset::<u64>::new())
            .with_initial_request(request.unwrap());

        let _ = client.dataset::<u64>();
        let _ = client.run().await?;
        Ok(())
    }
}
