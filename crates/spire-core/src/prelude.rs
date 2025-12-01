//! A convenience module that re-exports commonly used items.
//!
//! This module is intended to be glob-imported for convenience:
//!
//! ```ignore
//! use spire_core::prelude::*;
//! ```

#[doc(hidden)]
// Re-export async_trait for convenience
pub use crate::async_trait;
// Backend utilities with feature gates
#[cfg(feature = "metric")]
#[cfg_attr(docsrs, doc(cfg(feature = "metric")))]
pub use crate::backend::utils::Metric;
pub use crate::backend::utils::Noop;
#[cfg(feature = "tracing")]
#[cfg_attr(docsrs, doc(cfg(feature = "tracing")))]
pub use crate::backend::utils::Trace;
// Backend traits
pub use crate::backend::{Backend, Client as BackendClient, Worker};
// Context types
pub use crate::context::{
    Body, Context, Depth, FlowControl, IntoFlowControl, Request, RequestQueue, Response, Tag,
    TagQuery, Task, TaskBuilder, TaskExt,
};
// Dataset types
pub use crate::dataset::{
    Dataset, DatasetBulkExt, InMemDataset,
    future::{Data, DataSink, DataStream},
    utils::{BoxCloneDataset, BoxDataset, DatasetExt, MapData, MapErr},
};
// Core types and errors
pub use crate::{BoxError, Client, Error, ErrorKind, Result};
