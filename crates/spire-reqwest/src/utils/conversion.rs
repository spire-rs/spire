use spire_core::context::{Body, Request, Response};

/// Converts an http::Request to a reqwest::Request.
pub(crate) fn request_to_reqwest(req: Request) -> reqwest::Request {
    use reqwest::Client as RwClient;

    let (parts, _body) = req.into_parts();

    // TODO: Handle request body properly - requires async or streaming
    let body_bytes = bytes::Bytes::new();

    // Build URL from URI
    let url = parts.uri.to_string();

    // Build reqwest request
    RwClient::new()
        .request(parts.method, &url)
        .headers(parts.headers)
        .version(parts.version)
        .body(body_bytes)
        .build()
        .expect("failed to build request")
}

/// Converts a reqwest::Response to an http::Response.
pub(crate) fn response_from_reqwest(rw_res: reqwest::Response) -> Response {
    // Convert reqwest::Response to http::Response
    let mut res_builder = Response::builder()
        .status(rw_res.status())
        .version(rw_res.version());

    if let Some(headers) = res_builder.headers_mut() {
        *headers = rw_res.headers().clone();
    }

    // TODO: Handle response body properly - requires async streaming
    let body = Body::from(bytes::Bytes::new());

    res_builder.body(body).expect("failed to build response")
}
