//! Browser pool management and connection lifecycle.
//!
//! This module provides the core [`BrowserPool`] type and supporting infrastructure
//! for managing collections of WebDriver browser instances. It includes:
//!
//! - [`BrowserPool`] - Internal pool for managing browser connections
//! - [`BrowserBehaviorConfig`] - Configuration for browser behavior
//! - [`BrowserConnection`] - Individual browser connection wrapper

/// Browser pool builder for configuring and creating pools.
mod builder;
/// Browser configuration types.
mod config;
/// Browser connection implementations and pooled connection handling.
mod connection;
/// Internal browser connection manager and lifecycle handling.
mod manager;

pub use builder::BrowserBuilder;
pub use config::BrowserBehaviorConfig;
pub use connection::BrowserConnection;
pub use manager::{BrowserManager, BrowserPool};
