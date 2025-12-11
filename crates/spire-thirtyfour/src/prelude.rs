//! A convenience module that re-exports commonly used items.
//!
//! This module is intended to be glob-imported for convenience when you want
//! to access the most common types without individual imports:
//!
//! ```no_run
//! use spire_thirtyfour::prelude::*;
//! use spire_core::backend::Backend;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let backend = BrowserBackend::builder()
//!         .with_unmanaged("http://localhost:4444")
//!         .build()?;
//!
//!     let connection = backend.connect().await?;
//!     Ok(())
//! }
//! ```

pub use crate::{
    BrowserBackend, BrowserBehaviorConfig, BrowserBuilder, BrowserConfig, BrowserConfigBuilder,
    BrowserConnection, BrowserError, BrowserResult, NavigationErrorType,
};
