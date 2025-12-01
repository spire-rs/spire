//! A convenience module that re-exports commonly used items.
//!
//! This module is intended to be glob-imported for convenience:
//!
//! ```ignore
//! use spire::prelude::*;
//! ```

// Macros (with feature gate)
#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use spire_macros::Select as DeriveSelect;

#[doc(hidden)]
// Core async trait
pub use crate::async_trait;
// Re-export core spire-core types
pub use crate::backend::{Backend, Worker};
pub use crate::context::{
    Body, Context, Depth, IntoSignal, Request, RequestQueue, Response, Signal, Tag, TagQuery, Task,
    TaskBuilder,
};
pub use crate::dataset::future::{Data, DataSink, DataStream};
pub use crate::dataset::{Dataset, DatasetExt, InMemDataset};
// Extract traits and types
pub use crate::extract::{
    AttrData, AttrTag, Body as ExtractBody, Client as ExtractClient, Elements, FromContext,
    FromContextRef, Json, Select, State, Text,
};
// Backend implementations (with feature gates)
#[cfg(feature = "reqwest")]
#[cfg_attr(docsrs, doc(cfg(feature = "reqwest")))]
pub use crate::reqwest_backend;
#[cfg(feature = "thirtyfour")]
#[cfg_attr(docsrs, doc(cfg(feature = "thirtyfour")))]
pub use crate::thirtyfour_backend;
// Main types
pub use crate::{Client, Router};
// Core types and errors
pub use crate::{Error, ErrorKind, Result};
