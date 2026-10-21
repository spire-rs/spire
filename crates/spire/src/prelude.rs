//! A convenience module that re-exports commonly used items.
//!
//! ```
//! use spire::prelude::*;
//! ```

pub use crate::backend::{Backend, Client as _};
pub use crate::dataset::{Data, Dataset, InMemDataset};
#[cfg(feature = "html")]
pub use crate::extract::{Element, Elements, Html, Select};
pub use crate::extract::{FromRef, Json, State, Text};
pub use crate::frontier::{Admission, Frontier, FrontierExt, InMemFrontier, Link, Queue, Seed};
pub use crate::request::{Attempt, Depth, RequestExt as _, ResponseExt as _, Tag};
pub use crate::worker::{IntoOutcome, Outcome};
#[cfg(feature = "thirtyfour")]
pub use crate::{BrowserBackend, BrowserSession, extract::driver::View};
pub use crate::{Context, Crawler, Error, ErrorKind, Result, Router};
#[cfg(feature = "reqwest")]
pub use crate::{HttpClient, StatusPolicy};
