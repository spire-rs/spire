//! Requests, responses and their crawl metadata.

mod body;
mod meta;

pub use body::Body;
pub use meta::{Attempt, Depth, RequestExt, ResponseExt, Tag};

/// An HTTP request with a [`Body`].
pub type Request<B = Body> = http::Request<B>;

/// An HTTP response with a [`Body`].
pub type Response<B = Body> = http::Response<B>;
