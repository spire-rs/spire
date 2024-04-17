//! TODO.
//!

use tower::layer::util::Stack;
use tower::ServiceBuilder;

pub use exclude::{Exclude, ExcludeLayer};
pub use include::{Include, IncludeLayer};

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
    /// Conditionally rejects requests based on a fetched `robots.txt` file.
    fn exclude(self) -> ServiceBuilder<Stack<ExcludeLayer, L>>;

    /// TODO.
    fn include(self) -> ServiceBuilder<Stack<IncludeLayer, L>>;
}

impl<L> ServiceBuilderExt<L> for ServiceBuilder<L> {
    fn exclude(self) -> ServiceBuilder<Stack<ExcludeLayer, L>> {
        self.layer(ExcludeLayer::new())
    }

    fn include(self) -> ServiceBuilder<Stack<IncludeLayer, L>> {
        self.layer(IncludeLayer::new())
    }
}
