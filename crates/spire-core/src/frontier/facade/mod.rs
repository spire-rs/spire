//! Facades: frontiers that wrap another to add behavior, chained with
//! [`FrontierExt`](crate::frontier::FrontierExt).

mod backoff;
mod dedup;
mod depth;
mod filter;
mod retry;

pub use backoff::TagBackoff;
pub use dedup::Dedup;
pub use depth::MaxDepth;
pub use filter::{AllowedHosts, Filter};
pub use retry::{Retry, RetryPolicy};
