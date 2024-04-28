//! [`Worker`] middlewares.
//!
//! [`Worker`]: crate::backend::Worker

use tower::layer::util::Stack;
use tower::ServiceBuilder;

pub use exclude::{Exclude, ExcludeLayer};
pub use include::{Include, IncludeLayer};

mod exclude;
mod include;

pub mod futures {
    //! Future types for [`spire`] middlewares.
    //!
    //! [`spire`]: crate

    pub use exclude::ExcludeFuture;
    pub use include::IncludeFuture;

    use crate::middleware::exclude;
    use crate::middleware::include;
}

/// Extension trait for `tower::`[`ServiceBuilder`].
pub trait ServiceBuilderExt<L> {
    /// Conditionally rejects [`Request`]s based on a retrieved `robots.txt` file.
    ///
    /// [`Request`]: crate::context::Request
    fn exclude(self) -> ServiceBuilder<Stack<ExcludeLayer, L>>;

    /// Populates [`RequestQueue`] with [`Request`]s from a retrieved `sitemap.xml`.
    ///
    /// [`RequestQueue`]: crate::context::RequestQueue
    /// [`Request`]: crate::context::Request
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
