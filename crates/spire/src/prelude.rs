//! A convenience module that re-exports commonly used items.
//!
//! This module is intended to be glob-imported for convenience:
//!
//! ```
//! use spire::prelude::*;
//! ```

// Macros (with feature gate)
#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use spire_macros::Select as DeriveSelect;

#[cfg(feature = "reqwest")]
#[cfg_attr(docsrs, doc(cfg(feature = "reqwest")))]
pub use crate::HttpClient;
#[doc(hidden)]
pub use crate::async_trait;
pub use crate::backend::{Backend, Worker};
pub use crate::context::{
    Body, Context, Depth, FlowControl, IntoFlowControl, Request, RequestQueue, Response, Tag,
    TagQuery, Task, TaskBuilder,
};
pub use crate::dataset::future::{Data, DataSink, DataStream};
pub use crate::dataset::{Dataset, DatasetExt, InMemDataset};
pub use crate::extract::{
    AttrData, AttrTag, Body as ExtractBody, Client as ExtractClient, Elements, FromContext,
    FromContextRef, Json, Select, State, Text,
};
#[cfg(feature = "thirtyfour")]
#[cfg_attr(docsrs, doc(cfg(feature = "thirtyfour")))]
pub use crate::{
    BrowserBackend, BrowserConnection, BrowserError, BrowserPool, BrowserType, NavigationErrorType,
    PoolConfig, PoolConfigBuilder, WebDriverConfig, WebDriverConfigBuilder,
};
pub use crate::{Client, Error, ErrorKind, Result, Router};
