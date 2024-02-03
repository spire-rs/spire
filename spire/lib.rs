#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("./README.md")]

pub use spire_core::{backend, context, dataset, process};
#[cfg(feature = "macros")]
pub use spire_macros::*;

pub mod extract;
pub mod handler;
pub mod routing;
