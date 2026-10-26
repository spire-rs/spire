//! The crawl metadata of a response.

use http::Uri;

/// The crawl metadata of a response, stored as one extension: one
/// allocation per response instead of one per value.
#[derive(Debug, Clone, Default)]
pub(crate) struct ResponseMeta {
    /// The URI the response was served from, after redirects.
    pub(crate) final_uri: Option<Uri>,
}
