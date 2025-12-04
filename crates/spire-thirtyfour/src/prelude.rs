//! A convenience module that re-exports commonly used items.
//!
//! This module is intended to be glob-imported for convenience:
//!
//! ```
//! use spire_thirtyfour::prelude::*;
//! ```

pub use crate::config::capabilities::CapabilitiesBuilder;
pub use crate::pool::BrowserBuilder;
pub use crate::{
    BrowserBackend, BrowserConnection, BrowserError, BrowserType, ClientConfig,
    NavigationErrorType, PoolConfig, PoolConfigBuilder, WebDriverConfig, WebDriverConfigBuilder,
};
