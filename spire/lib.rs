#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("./README.md")]

#[doc(inline)]
pub use routing::Router;
pub use spire_core::{backend, context};
pub use spire_core::Daemon;
#[cfg(feature = "macros")]
pub use spire_macros::*;

pub mod extract;
pub mod handler;
pub mod routing;

#[doc(hidden)]
pub mod prelude {}
