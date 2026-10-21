//! Requests, responses and their crawl metadata.

mod meta;

use bytes::Bytes;
pub use meta::{Attempt, Depth, RequestExt, ResponseExt, Tag, Tags};

/// An HTTP request, with its body in memory.
pub type Request<B = Bytes> = http::Request<B>;

/// An HTTP response, with its body in memory, or without one: a client
/// returns the head, `Response<()>`, and its body separately.
pub type Response<B = Bytes> = http::Response<B>;
