//! Request source types for creating HTTP requests from various inputs.

use crate::Error;
use crate::context::{Body, Request};

/// A source that can be converted into a Request.
///
/// This enum allows creating requests from different sources like URLs or existing requests.
///
/// # Examples
///
/// ## Creating from a URL string
///
/// ```
/// # use spire_core::context::RequestSource;
/// let source = RequestSource::from("https://example.com");
/// ```
///
/// ## Creating from an existing Request
///
/// ```
/// # use spire_core::context::{Body, Request, RequestSource};
/// let request = Request::builder()
///     .uri("https://example.com")
///     .body(Body::default())
///     .unwrap();
/// let source = RequestSource::from(request);
/// ```
///
/// ## Using with RequestQueue methods
///
/// ```no_run
/// use spire_core::context::{RequestQueue, Tag, RequestSource};
/// use spire_core::dataset::InMemDataset;
/// use spire_core::dataset::utils::DatasetExt;
/// use std::num::NonZeroU32;
///
/// # async fn example() -> Result<(), spire_core::Error> {
/// // Create a request queue
/// let dataset = InMemDataset::queue().map_err(Into::into).boxed_clone();
/// let queue = RequestQueue::new(dataset, NonZeroU32::new(1).unwrap());
///
/// // Direct usage with strings - no need for explicit RequestSource conversion
/// queue.append("https://example.com").await?;
/// queue.append_with_tag(Tag::from("crawl"), "https://example.com").await?;
/// queue.branch_with_tag(Tag::from("detail"), "https://example.com/details").await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub enum RequestSource {
    /// An existing HTTP request
    Request(Request),
    /// A URL string that will be converted to a GET request
    Url(String),
}

impl From<Request> for RequestSource {
    fn from(request: Request) -> Self {
        Self::Request(request)
    }
}

impl From<String> for RequestSource {
    fn from(url: String) -> Self {
        Self::Url(url)
    }
}

impl From<&str> for RequestSource {
    fn from(url: &str) -> Self {
        Self::Url(url.to_string())
    }
}

impl TryFrom<RequestSource> for Request {
    type Error = Error;

    fn try_from(source: RequestSource) -> Result<Self, Self::Error> {
        match source {
            RequestSource::Request(request) => Ok(request),
            RequestSource::Url(url) => {
                use http::request::Builder;
                Builder::new()
                    .uri(url)
                    .body(Body::default())
                    .map_err(Into::into)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use http::Method;

    use super::*;

    #[test]
    fn test_request_source_from_str() {
        let source = RequestSource::from("https://example.com");
        matches!(source, RequestSource::Url(_));
    }

    #[test]
    fn test_request_source_from_string() {
        let url = String::from("https://example.com");
        let source = RequestSource::from(url);
        matches!(source, RequestSource::Url(_));
    }

    #[test]
    fn test_request_source_from_request() {
        let request = Request::builder()
            .uri("https://example.com")
            .body(Body::default())
            .unwrap();
        let source = RequestSource::from(request);
        matches!(source, RequestSource::Request(_));
    }

    #[test]
    fn test_request_source_try_into_request_from_url() {
        let source = RequestSource::Url("https://example.com".to_string());
        let request: Result<Request, Error> = source.try_into();
        assert!(request.is_ok());
        let request = request.unwrap();
        assert_eq!(request.method(), Method::GET);
        assert_eq!(request.uri(), "https://example.com");
    }

    #[test]
    fn test_request_source_try_into_request_from_request() {
        let original_request = Request::builder()
            .method(Method::POST)
            .uri("https://example.com/api")
            .body(Body::default())
            .unwrap();

        let source = RequestSource::Request(original_request);
        let request: Result<Request, Error> = source.try_into();
        assert!(request.is_ok());
        let request = request.unwrap();
        assert_eq!(request.method(), Method::POST);
        assert_eq!(request.uri(), "https://example.com/api");
    }

    #[test]
    fn test_request_source_try_into_request_invalid_url() {
        let source = RequestSource::Url("not a valid url with spaces".to_string());
        let request: Result<Request, Error> = source.try_into();
        assert!(request.is_err());
    }
}
