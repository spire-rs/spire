//! A convenience module that re-exports commonly used items.
//!
//! This module is intended to be glob-imported for convenience:
//!
//! ```ignore
//! use spire_reqwest::prelude::*;
//! ```

#[doc(hidden)]
pub use crate::{HttpClient, HttpConnection};
