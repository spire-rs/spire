use spire_core::request::Request;
use spire_core::response::{Response, ResponseExt};
use spire_core::{Error, Result, http};

/// Converts a request to a `reqwest::Request`, copying its body.
///
/// # Errors
///
/// Returns an error if the request URI is not a valid URL.
pub(crate) fn request_to_reqwest(request: &Request) -> Result<reqwest::Request> {
    let url = reqwest::Url::parse(&request.uri().to_string())
        .map_err(|error| Error::invalid("invalid request url").with_source(error))?;

    let mut converted = reqwest::Request::new(request.method().clone(), url);
    *converted.headers_mut() = request.headers().clone();
    *converted.version_mut() = request.version();
    if !request.body().is_empty() {
        *converted.body_mut() = Some(request.body().clone().into());
    }
    Ok(converted)
}

/// Returns the head of a `reqwest::Response`, recording the URL it was
/// served from after redirects.
#[must_use]
pub(crate) fn head_from_reqwest(response: &reqwest::Response) -> Response<()> {
    let mut head = http::Response::new(());
    *head.status_mut() = response.status();
    *head.version_mut() = response.version();
    *head.headers_mut() = response.headers().clone();
    if let Ok(uri) = response.url().as_str().parse::<http::Uri>() {
        head.set_final_uri(uri);
    }
    head
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    #[test]
    fn request_body_and_headers_are_copied() {
        let request = http::Request::post("https://example.com/submit")
            .header("x-test", "1")
            .body(Bytes::from_static(b"payload"))
            .unwrap();

        let converted = request_to_reqwest(&request).unwrap();
        assert_eq!(converted.method(), http::Method::POST);
        assert_eq!(converted.url().as_str(), "https://example.com/submit");
        assert_eq!(converted.headers()["x-test"], "1");
        let body = converted.body().and_then(reqwest::Body::as_bytes);
        assert_eq!(body, Some(&b"payload"[..]));
    }

    #[test]
    fn request_with_invalid_url_is_rejected() {
        let request = http::Request::get("/relative").body(Bytes::new()).unwrap();
        assert!(request_to_reqwest(&request).is_err());
    }

    #[test]
    fn response_head_keeps_status_headers_and_url() {
        let response = http::Response::builder()
            .status(http::StatusCode::CREATED)
            .header("content-type", "text/html")
            .body("<html></html>")
            .unwrap();

        let head = head_from_reqwest(&reqwest::Response::from(response));
        assert_eq!(head.status(), http::StatusCode::CREATED);
        assert!(head.final_uri().is_some());
        assert_eq!(head.headers()["content-type"], "text/html");
    }
}
