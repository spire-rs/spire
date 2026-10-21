#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(not(any(feature = "rustls", feature = "native-tls")))]
compile_error!("At least one TLS feature must be enabled: 'rustls' or 'native-tls'");

mod backend;
mod error;
mod pool;
mod session;

pub use thirtyfour;

pub use crate::backend::{BrowserBackend, BrowserBuilder};
pub use crate::session::BrowserSession;

/// Tracing target for browser backend operations.
pub const TRACING_TARGET: &str = "spire_thirtyfour";
