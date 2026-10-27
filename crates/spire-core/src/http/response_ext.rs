//! Responses and their crawl metadata.

use ::http::Uri;
use bytes::Bytes;

use super::meta::ResponseMeta;

/// An HTTP response with its body in memory by default. Sessions return
/// complete responses; helpers may use another body type, such as `()`.
pub type Response<B = Bytes> = ::http::Response<B>;

/// Reads and writes crawl metadata on a [`Response`].
pub trait ResponseExt {
    /// Returns the URI the response was served from after redirects, if the
    /// backend recorded it.
    ///
    /// Resolve relative links against this rather than the request URI.
    fn final_uri(&self) -> Option<&Uri>;

    /// Records the URI the response was served from; called by backends.
    fn set_final_uri(&mut self, uri: Uri);
}

impl<B> ResponseExt for ::http::Response<B> {
    fn final_uri(&self) -> Option<&Uri> {
        self.extensions().get::<ResponseMeta>()?.final_uri.as_ref()
    }

    fn set_final_uri(&mut self, uri: Uri) {
        let meta = self
            .extensions_mut()
            .get_or_insert_default::<ResponseMeta>();
        meta.final_uri = Some(uri);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_final_uri() {
        let mut response = Response::new(());
        assert_eq!(response.final_uri(), None);
        response.set_final_uri(Uri::from_static("https://example.com/moved"));
        assert_eq!(response.final_uri().unwrap(), "https://example.com/moved");
    }
}
