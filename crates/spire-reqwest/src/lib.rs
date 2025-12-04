#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod client;
mod utils;

#[doc(hidden)]
pub mod prelude;

pub use client::HttpClient;
pub use utils::{HttpService, client_to_service};
