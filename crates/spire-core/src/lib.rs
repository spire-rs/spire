#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod backend;
mod context;
pub mod crawler;
pub mod dataset;
mod error;
pub mod frontier;
pub mod request;
mod sync;
pub mod worker;

pub use http;

pub use crate::context::Context;
pub use crate::crawler::Crawler;
pub use crate::error::{BoxError, Error, ErrorKind, Result};

/// Tracing target for crawler operations.
const TRACING_TARGET: &str = "spire_core";
