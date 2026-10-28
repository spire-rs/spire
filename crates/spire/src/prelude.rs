//! A convenience module that re-exports commonly used items.
//!
//! ```
//! use spire::prelude::*;
//! ```

#[cfg(feature = "agent")]
pub use crate::agent::AgentCrawlerExt;
#[cfg(feature = "agent")]
pub use crate::agent::objective::ObjectiveExt;
#[cfg(feature = "reqwest")]
pub use crate::backend::ReqwestBackend;
pub use crate::backend::{Backend, Session as _, StatusPolicy};
#[cfg(feature = "thirtyfour")]
pub use crate::backend::{ThirtyfourBackend, ThirtyfourSession};
pub use crate::crawler::{Attempt, Depth, HostConcurrency, Tag, Tags};
pub use crate::dataset::{Dataset, InMemDataset};
#[cfg(feature = "agent")]
pub use crate::extract::AgentExtract;
#[cfg(feature = "thirtyfour")]
pub use crate::extract::ThirtyfourTab;
#[cfg(feature = "html")]
pub use crate::extract::html::{Element, Html, JsonLd, Meta, Select};
pub use crate::extract::{Body, Data, Extension, FromRef, Head, Json, Query, State, Text};
pub use crate::frontier::queue::{Link, Queue};
pub use crate::frontier::seed::Seed;
pub use crate::frontier::{Admission, Frontier, FrontierExt, InMemFrontier};
#[cfg(feature = "agent")]
pub use crate::handler::{AgentCollect, FollowRelevantLinks};
#[cfg(feature = "html")]
pub use crate::handler::{Collect, FollowLinks};
pub use crate::http::{RequestExt as _, ResponseExt as _};
pub use crate::worker::{IntoOutcome, Outcome, WorkerExt};
pub use crate::{Context, Crawler, Error, ErrorKind, Result, Router};
