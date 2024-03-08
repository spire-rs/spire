//! Various utility [`Dataset`]s.
//!
//! [`Dataset`]: crate::dataset::Dataset

pub use boxed::{BoxCloneDataset, BoxDataset};
pub use remap::{MapData, MapErr};

mod boxed;
mod remap;
