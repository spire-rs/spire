//! A convenience module that re-exports commonly used items.
//!
//! This module is intended to be glob-imported for convenience:
//!
//! ```
//! use spire_core::prelude::*;
//! ```

// HTTP types
pub use http;

#[doc(hidden)]
pub use crate::async_trait;
#[cfg(feature = "metric")]
#[cfg_attr(docsrs, doc(cfg(feature = "metric")))]
pub use crate::backend::utils::Metric;
pub use crate::backend::utils::Noop;
#[cfg(feature = "tracing")]
#[cfg_attr(docsrs, doc(cfg(feature = "tracing")))]
pub use crate::backend::utils::Trace;
pub use crate::backend::{Backend, Client as BackendClient, Worker};
pub use crate::context::{
    Body, Context, Depth, FlowControl, IntoFlowControl, Request, RequestQueue, Response, Tag,
    TagQuery, Task, TaskBuilder, TaskExt,
};
pub use crate::dataset::future::{Data, DataSink, DataStream};
pub use crate::dataset::utils::{BoxCloneDataset, BoxDataset, DatasetExt, MapData, MapErr};
pub use crate::dataset::{Dataset, DatasetBatchExt, InMemDataset};
pub use crate::{BoxError, Client, Error, ErrorKind, Result};
