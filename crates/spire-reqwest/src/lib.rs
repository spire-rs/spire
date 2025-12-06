#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

// Ensure at least one TLS feature is enabled
#[cfg(not(any(feature = "rustls-tls", feature = "native-tls")))]
compile_error!("At least one TLS feature must be enabled: 'rustls-tls' or 'native-tls'");

mod client;
mod utils;

// Re-export reqwest for convenience
pub use reqwest;

#[doc(hidden)]
pub mod prelude;

pub use client::HttpClient;
pub use utils::{HttpService, client_to_service};
