//! Crawl routing, depth, attempts and revisit schedules.

mod attempt;
mod depth;
mod revisit;
mod tag;

pub use attempt::Attempt;
pub use depth::Depth;
pub use revisit::Revisit;
pub use tag::{Tag, Tags};
