//! Browser pool management and connection lifecycle.
//!
//! This module provides infrastructure for managing collections of `WebDriver`
//! browser instances. It includes:
//!
//! - [`BrowserBehaviorConfig`] - Configuration for browser behavior
//! - [`BrowserConnection`] - Individual browser connection wrapper
//! - [`BrowserBuilder`] - Builder for configuring browser pools

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
// Keep manager types internal to this module
pub(crate) use manager::BrowserManager;
