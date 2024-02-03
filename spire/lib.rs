#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("./README.md")]

// pub use spire_core::{Builder, Collector};
// pub use spire_core::{Error, Result};

pub use routing::{Label, Router};
#[cfg(feature = "macros")]
pub use spire_macros::*;

pub mod extract;
pub mod handler;
mod routing;
