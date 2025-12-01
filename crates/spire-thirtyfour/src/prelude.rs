//! A convenience module that re-exports commonly used items.
//!
//! This module is intended to be glob-imported for convenience:
//!
//! ```ignore
//! use spire_thirtyfour::prelude::*;
//! ```

#[doc(hidden)]
pub use crate::config::capabilities::CapabilitiesBuilder;
#[doc(hidden)]
pub use crate::pool::builder::BrowserBuilder;
#[doc(hidden)]
pub use crate::{
    BrowserClient, BrowserError, BrowserPool, BrowserType, ClientConfig, NavigationErrorType,
    PoolConfig, PoolConfigBuilder, WebDriverConfig, WebDriverConfigBuilder,
};
