#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
mod backend;
mod client;
mod utils;

#[doc(hidden)]
pub mod prelude;

pub use backend::HttpClient;
pub use client::HttpConnection;
