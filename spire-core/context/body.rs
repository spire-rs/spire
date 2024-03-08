use std::any::Any;
use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body::{Body as HttpBody, Frame, SizeHint};
use http_body_util::{BodyExt, Empty};
use http_body_util::combinators::BoxBody;

use crate::BoxError;
use crate::context::Error;

/// Forked from [`axum_core`]`::body::Body`.
///
/// [`axum`]: https://github.com/tokio-rs/axum
fn try_downcast<T, K>(k: K) -> Result<T, K>
where
    T: 'static,
    K: Send + 'static,
{
    let mut k = Some(k);
    match <dyn Any>::downcast_mut::<Option<T>>(&mut k) {
        Some(k) => Ok(k.take().unwrap()),
        _ => Err(k.unwrap()),
    }
}

/// The `http::`[`Body`] type used in [`Request`]s and [`Response`]s.
///
/// [`Body`]: http_body::Body
/// [`Request`]: http::Request
/// [`Response`]: http::Response
pub struct Body(BoxBody<Bytes, Error>);

impl Body {
    /// Creates a new [`Body`].
    pub fn new<B>(body: B) -> Self
    where
        B: HttpBody<Data = Bytes> + Send + Sync + 'static,
        B::Error: Into<BoxError>,
    {
        try_downcast(body).unwrap_or_else(|x| Self(x.map_err(Error::new).boxed()))
    }
}

impl Clone for Body {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl Default for Body {
    fn default() -> Self {
        Self::new(Empty::new())
    }
}

impl From<()> for Body {
    fn from(_: ()) -> Self {
        Self::default()
    }
}

impl fmt::Debug for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Body").finish_non_exhaustive()
    }
}

impl HttpBody for Body {
    type Data = Bytes;
    type Error = Error;

    #[inline]
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.0).poll_frame(cx)
    }

    #[inline]
    fn is_end_stream(&self) -> bool {
        self.0.is_end_stream()
    }

    #[inline]
    fn size_hint(&self) -> SizeHint {
        self.0.size_hint()
    }
}
