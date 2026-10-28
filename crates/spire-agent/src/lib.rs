#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod agent;
mod budget;
pub mod cache;
mod crawler;
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
pub use budget::Budget;
pub use crawler::AgentCrawlerExt;
pub use error::{Error, Result};
pub use provider::{Provider, ProviderRequest, ProviderResponse};
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
