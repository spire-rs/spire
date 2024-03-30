//! TODO.
//!

use tower::layer::util::Stack;
use tower::ServiceBuilder;

pub use exclude::{Exclude, ExcludeLayer};
pub use include::{Include, IncludeLayer};

mod block;
mod defer;
mod exclude;
mod include;

pub mod futures {
    //! TODO.
    //!

    pub use exclude::ExcludeFuture;
    pub use include::IncludeFuture;

    use crate::middleware::exclude;
    use crate::middleware::include;
}

/// TODO.
pub trait ServiceBuilderExt<L> {
    /// TODO.
    fn exclude(self) -> ServiceBuilder<Stack<ExcludeLayer, L>>;
}

impl<L> ServiceBuilderExt<L> for ServiceBuilder<L> {
    fn exclude(self) -> ServiceBuilder<Stack<ExcludeLayer, L>> {
        todo!()
    }
}
