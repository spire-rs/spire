#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(not(any(feature = "rustls-tls", feature = "native-tls")))]
compile_error!("At least one TLS feature must be enabled: 'rustls-tls' or 'native-tls'");

mod client;
mod clients;
mod conversion;

pub use reqwest;
pub use spire_core::backend::StatusPolicy;

pub use crate::client::{ReqwestBackend, ReqwestSession};
