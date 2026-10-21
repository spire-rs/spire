use spire_core::request::{Body, Request, Response, ResponseExt};
use spire_core::{Error, Result, http};

/// Converts an `http::Request` to a `reqwest::Request`.
///
/// The request body is streamed through without buffering.
///
/// # Errors
///
/// Returns an error if the request URI is not a valid URL.
pub(crate) fn request_to_reqwest(req: Request) -> Result<reqwest::Request> {
    let req = req.map(reqwest::Body::wrap);
    reqwest::Request::try_from(req)
        .map_err(|error| Error::invalid("invalid request").with_source(error))
}

/// Converts a `reqwest::Response` to an `http::Response`, recording the URL
/// it was served from after redirects.
///
/// The response body is streamed through without buffering.
#[must_use]
pub(crate) fn response_from_reqwest(rw_res: reqwest::Response) -> Response {
    let final_uri = rw_res.url().as_str().parse::<http::Uri>();
    let mut response = http::Response::<reqwest::Body>::from(rw_res).map(Body::new);
    if let Ok(uri) = final_uri {
        response.set_final_uri(uri);
    }
    response
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use http_body_util::BodyExt;

    use super::*;

    #[tokio::test]
    async fn request_body_is_preserved() {
        let req = http::Request::post("https://example.com/submit")
            .header("x-test", "1")
            .body(Body::from("payload"))
            .unwrap();

        let mut rw_req = request_to_reqwest(req).unwrap();
        assert_eq!(rw_req.method(), http::Method::POST);
        assert_eq!(rw_req.url().as_str(), "https://example.com/submit");
        assert_eq!(rw_req.headers()["x-test"], "1");

        let body = rw_req.body_mut().take().unwrap();
        let bytes = body.collect().await.unwrap().to_bytes();
        assert_eq!(bytes, Bytes::from_static(b"payload"));
    }

    #[test]
    fn request_with_invalid_url_is_rejected() {
        let req = http::Request::get("/relative")
            .body(Body::default())
            .unwrap();
        assert!(request_to_reqwest(req).is_err());
    }

    #[tokio::test]
    async fn response_body_is_preserved() {
        let res = http::Response::builder()
            .status(http::StatusCode::CREATED)
            .header("content-type", "text/html")
            .body("<html></html>")
            .unwrap();

        let res = response_from_reqwest(reqwest::Response::from(res));
        assert_eq!(res.status(), http::StatusCode::CREATED);
        assert!(res.final_uri().is_some());
        assert_eq!(res.headers()["content-type"], "text/html");

        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(bytes, Bytes::from_static(b"<html></html>"));
    }
}
