//! [`Backend`] and [`Worker`] middlewares.
//!
//! [`Backend`]: crate::backend::Backend
//! [`Worker`]: crate::backend::Worker

#[cfg(any(
    feature = "exclude",
    feature = "include",
    feature = "metric",
    feature = "trace"
))]
use tower::layer::util::Stack;
use tower::ServiceBuilder;

#[cfg(feature = "exclude")]
#[cfg_attr(docsrs, doc(cfg(feature = "exclude")))]
pub use exclude::{Exclude, ExcludeLayer};
#[cfg(feature = "include")]
#[cfg_attr(docsrs, doc(cfg(feature = "include")))]
pub use include::{Include, IncludeLayer};

#[cfg(feature = "metric")]
use crate::backend::utils::MetricLayer;
#[cfg(feature = "trace")]
use crate::backend::utils::TraceLayer;

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
    #[cfg(feature = "metric")]
    #[cfg_attr(docsrs, doc(cfg(feature = "metric")))]
    fn metric(self) -> ServiceBuilder<Stack<MetricLayer, L>>;

    /// Enables tracing middleware for improved observability.
    #[cfg(feature = "trace")]
    #[cfg_attr(docsrs, doc(cfg(feature = "trace")))]
    fn trace(self) -> ServiceBuilder<Stack<TraceLayer, L>>;

    /// Conditionally rejects [`Request`]s based on a retrieved `robots.txt` file.
    ///
    /// [`Request`]: crate::context::Request
    #[cfg(feature = "exclude")]
    #[cfg_attr(docsrs, doc(cfg(feature = "exclude")))]
    fn exclude(self) -> ServiceBuilder<Stack<ExcludeLayer, L>>;

    /// Populates [`RequestQueue`] with [`Request`]s from a retrieved `sitemap.xml` file.
    ///
    /// [`RequestQueue`]: crate::context::RequestQueue
    /// [`Request`]: crate::context::Request
    #[cfg(feature = "include")]
    #[cfg_attr(docsrs, doc(cfg(feature = "include")))]
    fn include(self) -> ServiceBuilder<Stack<IncludeLayer, L>>;
}

impl<L> ServiceBuilderExt<L> for ServiceBuilder<L> {
    #[cfg(feature = "metric")]
    fn metric(self) -> ServiceBuilder<Stack<MetricLayer, L>> {
        self.layer(MetricLayer::new())
    }

    #[cfg(feature = "trace")]
    fn trace(self) -> ServiceBuilder<Stack<TraceLayer, L>> {
        self.layer(TraceLayer::new())
    }

    #[cfg(feature = "exclude")]
    fn exclude(self) -> ServiceBuilder<Stack<ExcludeLayer, L>> {
        self.layer(ExcludeLayer::new())
    }

    #[cfg(feature = "include")]
    fn include(self) -> ServiceBuilder<Stack<IncludeLayer, L>> {
        self.layer(IncludeLayer::new())
    }
}
