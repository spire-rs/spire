#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(not(any(feature = "rustls-tls", feature = "native-tls")))]
compile_error!("At least one TLS feature must be enabled: 'rustls-tls' or 'native-tls'");

mod backend;
mod error;
mod session;

pub use spire_core::backend::StatusPolicy;
pub use thirtyfour;

pub use crate::backend::{ThirtyfourBackend, ThirtyfourBuilder};
pub use crate::session::ThirtyfourSession;

/// Tracing target for browser backend operations.
const TRACING_TARGET: &str = "spire_thirtyfour";
