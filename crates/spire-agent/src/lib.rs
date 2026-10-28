#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod agent;
mod budget;
pub mod cache;
mod error;
pub mod objective;
mod provider;
#[cfg(feature = "rig")]
mod rig_provider;
#[cfg(feature = "testing")]
#[cfg_attr(docsrs, doc(cfg(feature = "testing")))]
pub mod testing;
mod usage;

pub use agent::Agent;
pub use budget::{Budget, Pricing};
pub use error::{Error, Result};
pub use provider::{CompletionRequest, CompletionResponse, Provider};
#[cfg(feature = "rig")]
#[cfg_attr(docsrs, doc(cfg(feature = "rig")))]
#[doc(no_inline)]
pub use rig;
#[cfg(feature = "rig")]
#[cfg_attr(docsrs, doc(cfg(feature = "rig")))]
pub use rig_provider::Rig;
pub use schemars;
pub use serde;
pub use serde_json;
pub use usage::{Usage, UsageTotals};

/// Model-extracted data.
///
/// Use `Vec<T>` for zero or more records, or `Option<T>` for a record that may
/// be absent. The Spire facade's `agent` feature implements its extractor
/// integration; this crate accepts text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extract<T>(pub T);
