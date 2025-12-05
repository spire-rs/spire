//! Browser pool management and connection lifecycle.
//!
//! This module provides the core [`BrowserBackend`] and [`BrowserPool`] types and supporting infrastructure
//! for managing collections of WebDriver browser instances. It includes:
//!
//! - [`BrowserBackend`] - Main backend interface implementing the Spire Backend trait
//! - [`BrowserPool`] - Internal pool for managing browser connections

/// Browser pool builder for configuring and creating pools.
mod builder;
/// Browser connection implementations and pooled connection handling.
mod connection;
/// Internal browser connection manager and lifecycle handling.
mod manager;

pub use builder::BrowserBuilder;
pub use connection::BrowserConnection;
pub use manager::{BrowserPool, WebDriverManager};
