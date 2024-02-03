use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body::Frame;

/// The [`Body`] type used in [`Request`]s and [`Response`]s.
///
/// [`Body`]: http_body::Body
/// [`Request`]: http::Request
/// [`Response`]: http::Response
pub struct Body {}

impl Body {
    pub fn new() -> Self {
        Self {}
    }

    pub fn empty() -> Self {
        Self {}
    }
}

impl Default for Body {
    fn default() -> Self {
        Self::empty()
    }
}

impl http_body::Body for Body {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        todo!()
    }
}
