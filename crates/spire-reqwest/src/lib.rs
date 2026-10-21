#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(not(any(feature = "rustls", feature = "native-tls")))]
compile_error!("At least one TLS feature must be enabled: 'rustls' or 'native-tls'");

mod client;
mod conversion;
mod status;

pub use reqwest;

pub use crate::client::HttpClient;
pub use crate::status::StatusPolicy;
