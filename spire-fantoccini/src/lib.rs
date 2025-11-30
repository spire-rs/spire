//! Fantoccini-based browser automation backend for Spire.
//!
//! This crate provides [`BrowserPool`], a WebDriver-based browser automation backend
//! that integrates with the Spire web scraping framework using the Fantoccini library.
//!
//! # Examples
//!
//! ```ignore
//! use spire_fantoccini::BrowserPool;
//! use spire_core::Client;
//!
//! let pool = BrowserPool::builder()
//!     .with_unmanaged("127.0.0.1:4444")
//!     .build();
//!
//! let client = Client::new(pool, worker);
//! ```

mod builder;
mod client;
mod connect;
mod manager;
mod pool;

pub use builder::BrowserBuilder;
pub use client::BrowserClient;
pub use pool::BrowserPool;
