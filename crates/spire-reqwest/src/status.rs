use std::time::{Duration, SystemTime};

use spire_core::http::header::RETRY_AFTER;
use spire_core::http::{HeaderMap, StatusCode};
use spire_core::request::{Response, ResponseExt};
use spire_core::{Error, ErrorKind, Result};

/// How an [`HttpClient`] treats error statuses.
///
/// [`HttpClient`]: crate::HttpClient
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum StatusPolicy {
    /// `4xx` and `5xx` responses become errors, so the crawler retries or
    /// fails them without a handler having to check:
    ///
    /// - `408`, `429` and `5xx` are retryable [`Http`] errors; `429` and
    ///   `503` honour `Retry-After` by pausing the request's tag.
    /// - Other `4xx` are [`Rejected`] errors, which are not retried.
    ///
    /// [`Http`]: ErrorKind::Http
    /// [`Rejected`]: ErrorKind::Rejected
    #[default]
    Classify,
    /// Every response is returned, whatever its status.
    Accept,
}

impl StatusPolicy {
    /// Applies the policy to a response.
    pub(crate) fn apply(self, response: Response) -> Result<Response> {
        let status = response.status();
        if self == Self::Accept || !(status.is_client_error() || status.is_server_error()) {
            return Ok(response);
        }

        let retryable = status.is_server_error()
            || matches!(
                status,
                StatusCode::REQUEST_TIMEOUT | StatusCode::TOO_MANY_REQUESTS
            );
        let kind = if retryable {
            ErrorKind::Http
        } else {
            ErrorKind::Rejected
        };

        let message = match response.final_uri() {
            Some(uri) => format!("{uri} responded with {status}"),
            None => format!("responded with {status}"),
        };
        let mut error = Error::new(kind).with_message(message).with_status(status);
        if let Some(uri) = response.final_uri() {
            error = error.with_uri(uri.clone());
        }

        let honours_retry_after = matches!(
            status,
            StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE
        );
        if honours_retry_after && let Some(delay) = retry_after(response.headers()) {
            error = error.with_retry_after(delay);
        }
        Err(error)
    }
}

/// Parses a `Retry-After` header given in seconds or as an HTTP date.
fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }

    let at = httpdate::parse_http_date(value).ok()?;
    Some(at.duration_since(SystemTime::now()).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use spire_core::http::{HeaderValue, Uri};

    use super::*;

    fn response(status: u16, retry_after: Option<&'static str>) -> Response {
        let mut response = Response::default();
        *response.status_mut() = StatusCode::from_u16(status).unwrap();
        response.set_final_uri(Uri::from_static("https://example.com/"));
        if let Some(value) = retry_after {
            let value = HeaderValue::from_static(value);
            response.headers_mut().insert(RETRY_AFTER, value);
        }
        response
    }

    fn classify(status: u16, retry_after: Option<&'static str>) -> Error {
        StatusPolicy::Classify
            .apply(response(status, retry_after))
            .unwrap_err()
    }

    #[test]
    fn success_and_redirects_pass() {
        for status in [200, 204, 301, 304] {
            assert!(StatusPolicy::Classify.apply(response(status, None)).is_ok());
        }
    }

    #[test]
    fn classifies_error_statuses() {
        assert_eq!(classify(404, None).kind(), ErrorKind::Rejected);
        assert_eq!(classify(403, None).kind(), ErrorKind::Rejected);
        assert_eq!(classify(408, None).kind(), ErrorKind::Http);
        assert_eq!(classify(500, None).kind(), ErrorKind::Http);

        let error = classify(502, None);
        assert_eq!(error.status(), Some(StatusCode::BAD_GATEWAY));
        assert_eq!(error.uri().unwrap(), "https://example.com/");
        assert_eq!(
            error.to_string(),
            "https://example.com/ responded with 502 Bad Gateway"
        );
    }

    #[test]
    fn honours_retry_after() {
        let error = classify(429, Some("120"));
        assert!(error.is_retryable());
        assert_eq!(error.retry_after(), Some(Duration::from_secs(120)));

        let past = classify(503, Some("Wed, 21 Oct 2015 07:28:00 GMT"));
        assert_eq!(past.retry_after(), Some(Duration::ZERO));

        // Only rate limiting and unavailability carry a pause.
        assert_eq!(classify(500, Some("120")).retry_after(), None);
        assert_eq!(classify(429, Some("soon")).retry_after(), None);
    }

    #[test]
    fn accept_returns_everything() {
        assert!(StatusPolicy::Accept.apply(response(500, None)).is_ok());
    }
}
