//! Utility functions and type aliases for the spire-reqwest crate.

mod conversion;
mod service;

pub(crate) use conversion::{request_to_reqwest, response_from_reqwest};
pub use service::{HttpService, client_to_service};
