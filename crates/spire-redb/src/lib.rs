#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod codec;
mod dataset;
mod frontier;
mod seen;
mod store;
mod validators;

pub use redb;

pub use crate::dataset::RedbDataset;
pub use crate::frontier::RedbFrontier;
pub use crate::seen::RedbSeen;
pub use crate::store::{RedbStore, RedbStoreBuilder};
pub use crate::validators::RedbValidators;

/// Tracing target for redb storage.
const TRACING_TARGET: &str = "spire_redb";
