#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("./README.md")]

// pub use collect::{Builder, Collector, CollectorContext, Label};
// pub use collector::{Error, Result};

mod collect;
pub mod macros;
mod router;
mod worker;
mod driver;
