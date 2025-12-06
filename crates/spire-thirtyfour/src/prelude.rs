//! A convenience module that re-exports commonly used items.
//!
//! This module is intended to be glob-imported for convenience:
//!
//! ```
//! use spire_thirtyfour::prelude::*;
//! ```

pub use crate::pool::{BrowserBehaviorConfig, BrowserBuilder};
pub use crate::{
    BrowserBackend, BrowserConfig, BrowserConfigBuilder, BrowserConnection, BrowserError,
    NavigationErrorType, PoolConfig, PoolConfigBuilder,
};
