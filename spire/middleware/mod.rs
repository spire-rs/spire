//! [`Worker`] middlewares.
//!
//! [`Worker`]: crate::backend::Worker

#[cfg(any(feature = "exclude", feature = "include"))]
use tower::layer::util::Stack;
use tower::ServiceBuilder;

#[cfg(feature = "exclude")]
#[cfg_attr(docsrs, doc(cfg(feature = "exclude")))]
pub use exclude::{Exclude, ExcludeLayer};
#[cfg(feature = "include")]
#[cfg_attr(docsrs, doc(cfg(feature = "include")))]
pub use include::{Include, IncludeLayer};

#[cfg(feature = "exclude")]
mod exclude;
#[cfg(feature = "include")]
mod include;

pub mod futures {
    //! Future types for [`spire`] middlewares.
    //!
    //! [`spire`]: crate

    #[cfg(feature = "exclude")]
    #[cfg_attr(docsrs, doc(cfg(feature = "exclude")))]
    pub use exclude::ExcludeFuture;
    #[cfg(feature = "include")]
    #[cfg_attr(docsrs, doc(cfg(feature = "include")))]
    pub use include::IncludeFuture;

    #[cfg(feature = "exclude")]
    use crate::middleware::exclude;
    #[cfg(feature = "include")]
    use crate::middleware::include;
}

/// Extension trait for `tower::`[`ServiceBuilder`].
pub trait ServiceBuilderExt<L> {
    /// Conditionally rejects [`Request`]s based on a retrieved `robots.txt` file.
    ///
    /// [`Request`]: crate::context::Request
    #[cfg(feature = "exclude")]
    #[cfg_attr(docsrs, doc(cfg(feature = "exclude")))]
    fn exclude(self) -> ServiceBuilder<Stack<ExcludeLayer, L>>;

    /// Populates [`RequestQueue`] with [`Request`]s from a retrieved `sitemap.xml`.
    ///
    /// [`RequestQueue`]: crate::context::RequestQueue
    /// [`Request`]: crate::context::Request
    #[cfg(feature = "include")]
    #[cfg_attr(docsrs, doc(cfg(feature = "include")))]
    fn include(self) -> ServiceBuilder<Stack<IncludeLayer, L>>;
}

impl<L> ServiceBuilderExt<L> for ServiceBuilder<L> {
    #[cfg(feature = "exclude")]
    fn exclude(self) -> ServiceBuilder<Stack<ExcludeLayer, L>> {
        self.layer(ExcludeLayer::new())
    }

    #[cfg(feature = "include")]
    fn include(self) -> ServiceBuilder<Stack<IncludeLayer, L>> {
        self.layer(IncludeLayer::new())
    }
}
