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
pub use crate::dataset::{Data, Dataset, InMemDataset};
#[cfg(feature = "thirtyfour")]
pub use crate::extract::View;
#[cfg(feature = "html")]
pub use crate::extract::{Element, Html, Select};
pub use crate::extract::{FromRef, Json, State, Text};
pub use crate::frontier::memory::InMemFrontier;
pub use crate::frontier::queue::{Link, Queue};
pub use crate::frontier::seed::Seed;
pub use crate::frontier::{Admission, Frontier, FrontierExt};
pub use crate::request::{Attempt, Depth, RequestExt as _, ResponseExt as _, Tag, Tags};
pub use crate::worker::{IntoOutcome, Outcome, WorkerExt};
pub use crate::{Context, Crawler, Error, ErrorKind, HostDelay, Result, Router};
