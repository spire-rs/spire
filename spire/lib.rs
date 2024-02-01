#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("./README.md")]

pub use routing::{Label, Router};
pub use spire_core::{Error, Result};

pub mod extract;
pub mod handler;
mod routing;
