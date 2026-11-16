//! Facades: frontiers that wrap another to add behavior, chained with
//! [`FrontierExt`](crate::frontier::FrontierExt).

mod dedup;
mod depth;
mod filter;
mod retry;
mod revisit;
mod score;
mod traps;

pub use dedup::Dedup;
pub use depth::MaxDepth;
pub use filter::{AllowedHosts, Filter};
pub use retry::{Retry, RetryPolicy};
pub use revisit::Revisits;
pub use score::Scored;
pub use traps::{AvoidTraps, TrapPolicy};
