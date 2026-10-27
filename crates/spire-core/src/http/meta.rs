//! Metadata stored in HTTP extensions, private to the implementation.

use ::http::Uri;

use crate::crawler::{Attempt, Depth, Tag};
use crate::identity::Identity;

/// The crawl metadata of a request, stored as one extension: one allocation
/// per request instead of one per value.
#[derive(Debug, Clone, Default)]
pub(crate) struct RequestMeta {
    pub(crate) tag: Option<Tag>,
    pub(crate) depth: Depth,
    pub(crate) attempt: Attempt,
    /// Picked by the crawl when the request starts, after the rest.
    pub(crate) identity: Option<Identity>,
}

/// The crawl metadata of a response, stored as one extension: one
/// allocation per response instead of one per value.
#[derive(Debug, Clone, Default)]
pub(crate) struct ResponseMeta {
    /// The URI the response was served from, after redirects.
    pub(crate) final_uri: Option<Uri>,
}
