use std::any::Any;
use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body::{Body as HttpBody, Frame, SizeHint};
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, Full};

use crate::BoxError;

/// Forked from [`axum_core`]`::body::Body`.
///
/// [`axum_core`]: https://github.com/tokio-rs/axum
fn try_downcast<T, K>(k: K) -> Result<T, K>
where
    T: 'static,
    K: Send + 'static,
{
    let mut k = Some(k);
    match <dyn Any>::downcast_mut::<Option<T>>(&mut k) {
        Some(x) => Ok(x.take().unwrap()),
        _ => Err(k.unwrap()),
    }
}

/// The `http::`[`Body`] type used in [`Request`]s and [`Response`]s.
///
/// [`Body`]: http_body::Body
/// [`Request`]: http::Request
/// [`Response`]: http::Response
pub struct Body(BoxBody<Bytes, BoxError>);

impl Body {
    /// Creates a new [`Body`].
    pub fn new<B>(body: B) -> Self
    where
        B: HttpBody<Data = Bytes> + Send + Sync + 'static,
        B::Error: Into<BoxError> + Send + Sync + 'static,
    {
        try_downcast(body).unwrap_or_else(|x| {
            let boxed = x.map_err(|x| x.into()).boxed();
            Self(boxed)
        })
    }
}

impl Default for Body {
    #[inline]
    fn default() -> Self {
        Self::new(Empty::new())
    }
}

impl From<()> for Body {
    #[inline]
    fn from(_: ()) -> Self {
        Self::default()
    }
}

impl From<Bytes> for Body {
    #[inline]
    fn from(bytes: Bytes) -> Self {
        Self::new(Full::new(bytes))
    }
}

impl From<Vec<u8>> for Body {
    #[inline]
    fn from(vec: Vec<u8>) -> Self {
        Bytes::from(vec).into()
    }
}

impl From<&'static [u8]> for Body {
    #[inline]
    fn from(slice: &'static [u8]) -> Self {
        Bytes::from(slice).into()
    }
}

impl From<String> for Body {
    #[inline]
    fn from(string: String) -> Self {
        string.into_bytes().into()
    }
}

impl From<&'static str> for Body {
    #[inline]
    fn from(slice: &'static str) -> Body {
        slice.as_bytes().into()
    }
}

impl fmt::Debug for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Body").finish_non_exhaustive()
    }
}

impl HttpBody for Body {
    type Data = Bytes;
    type Error = BoxError;

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

/// Type alias for `http::`[`Request`] whose body type defaults to [`Body`].
///
/// [`Request`]: http::Request
pub type Request<B = Body> = http::Request<B>;

/// Type alias for `http::`[`Response`] whose body type defaults to [`Body`].
///
/// [`Response`]: http::Response
pub type Response<B = Body> = http::Response<B>;

#[cfg(test)]
mod test {
    use bytes::Bytes;
    use http_body_util::{Empty, Full};

    use crate::context::Body;

    #[test]
    pub fn empty() {
        let _ = Body::default();
        let _ = Body::new(Empty::new());
    }

    #[test]
    pub fn full() {
        let body = Full::new(Bytes::new());
        let _ = Body::new(body);
    }

    #[test]
    pub fn stream() {
        // TODO: Streaming body.
    }
}
