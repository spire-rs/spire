//! A convenience module that re-exports commonly used items.
//!
//! ```
//! use spire::prelude::*;
//! ```

#[cfg(feature = "reqwest")]
pub use crate::backend::HttpClient;
pub use crate::backend::{Backend, Client as _, StatusPolicy};
#[cfg(feature = "thirtyfour")]
pub use crate::backend::{BrowserBackend, BrowserSession};
pub use crate::crawler::HostConcurrency;
pub use crate::dataset::{Data, Dataset, InMemDataset};
#[cfg(feature = "thirtyfour")]
pub use crate::extract::Tab;
#[cfg(feature = "html")]
pub use crate::extract::html::{Element, Html, JsonLd, Meta, Select};
pub use crate::extract::{Body, FromRef, Head, Json, Query, State, Text};
pub use crate::frontier::queue::{Link, Queue};
pub use crate::frontier::seed::Seed;
pub use crate::frontier::{Admission, Frontier, FrontierExt, InMemFrontier};
#[cfg(feature = "html")]
pub use crate::handler::{Collect, FollowLinks};
pub use crate::request::{Attempt, Depth, RequestExt as _, Tag, Tags};
pub use crate::response::ResponseExt as _;
pub use crate::worker::{IntoOutcome, Outcome, WorkerExt};
pub use crate::{Context, Crawler, Error, ErrorKind, Result, Router};
