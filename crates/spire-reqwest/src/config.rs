use std::time::Duration;

use reqwest::header::HeaderMap;

/// Configuration for one reqwest client, recreated for each crawl identity.
///
/// Spire owns the total fetch timeout and default headers so they can be
/// applied correctly across redirect hops.
#[derive(Debug, Default)]
pub struct ClientConfig {
    pub(crate) transport: reqwest::ClientBuilder,
    pub(crate) timeout: Option<Duration>,
    pub(crate) headers: HeaderMap,
}

impl ClientConfig {
    /// Creates a configuration with reqwest's transport defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures TLS, proxies, connection timeouts and other transport
    /// options.
    ///
    /// Set total timeouts and default headers with [`Self::timeout`] and
    /// [`Self::default_headers`], rather than on the transport builder.
    /// Redirect policies and cookie providers are managed by Spire.
    #[must_use]
    pub fn transport(
        mut self,
        configure: impl FnOnce(reqwest::ClientBuilder) -> reqwest::ClientBuilder,
    ) -> Self {
        self.transport = configure(self.transport);
        self
    }

    /// Limits the complete fetch, including redirect admission, every hop,
    /// and the final response body. By default there is no total timeout.
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Adds defaults to the initial request without overriding its headers.
    /// Redirects remove credentials and body headers when appropriate.
    #[must_use]
    pub fn default_headers(mut self, headers: HeaderMap) -> Self {
        self.headers.extend(headers);
        self
    }
}
