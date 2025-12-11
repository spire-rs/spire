//! A convenience module that re-exports commonly used items.
//!
//! This module is intended to be glob-imported for convenience:
//!
//! ```
//! use spire::prelude::*;
//! ```

// HTTP types
pub use spire_core::http;
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
#[cfg(feature = "reqwest")]
#[cfg_attr(docsrs, doc(cfg(feature = "reqwest")))]
pub use crate::extract::client::HttpBackend;
pub use crate::extract::{
    AttrData, AttrTag, Body as ExtractBody, Client as ExtractClient, Elements, FromContext,
    FromContextRef, Html, Json, Select, State, Text,
};
#[cfg(feature = "thirtyfour")]
#[cfg_attr(docsrs, doc(cfg(feature = "thirtyfour")))]
pub use crate::{
    BrowserBackend, BrowserBehaviorConfig, BrowserBuilder, BrowserConfig, BrowserConfigBuilder,
    BrowserConnection, BrowserError, BrowserResult, NavigationErrorType, thirtyfour,
};
pub use crate::{Client, Error, ErrorKind, Result, Router};
