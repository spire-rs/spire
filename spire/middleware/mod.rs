//! TODO.
//!

use tower::layer::util::Stack;
use tower::ServiceBuilder;

pub use exclude::{Exclude, ExcludeLayer};

mod exclude;
mod include;

pub mod futures {
    //! TODO.
    //!

    pub use exclude::ExcludeFuture;

    use crate::middleware::exclude;
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
