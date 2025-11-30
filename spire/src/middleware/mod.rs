//! Middleware for composing request processing pipelines.
//!
//! This module provides middleware components that can be applied to routes
//! using the [`Router::layer`] method. Middleware can modify requests, responses,
//! add logging, implement rate limiting, and more.
//!
//! # Using Middleware
//!
//! Middleware is applied using the [`Layer`] trait from the `tower` ecosystem:
//!
//! ```ignore
//! use spire::Router;
//! use tower::ServiceBuilder;
//!
//! let router = Router::new()
//!     .route(tag, handler)
//!     .layer(
//!         ServiceBuilder::new()
//!             .layer(my_middleware_layer)
//!     );
//! ```
//!
//! # Tower Ecosystem
//!
//! Spire is built on the [`tower`] service abstraction, which means you can use
//! any middleware from the tower ecosystem:
//!
//! - [`tower::timeout`] - Request timeouts
//! - [`tower::limit`] - Rate limiting and concurrency control
//! - [`tower::buffer`] - Request buffering
//! - [`tower::retry`] - Automatic retries
//!
//! # Custom Middleware
//!
//! You can create custom middleware by implementing the [`Layer`] and [`Service`]
//! traits from tower. See the [tower documentation] for more details.
//!
//! [`Router::layer`]: crate::Router::layer
//! [`Layer`]: tower::Layer
//! [`Service`]: tower::Service
//! [`tower`]: https://docs.rs/tower
//! [tower documentation]: https://docs.rs/tower/latest/tower/

// Placeholder for future middleware implementations
