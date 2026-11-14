//! HTTP redirect resolution and header handling.

use spire_core::backend::redirect::{Redirects, Transfer};
use spire_core::http::Request;
use spire_core::http::http::header::{
    AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, COOKIE, HOST, LOCATION, PROXY_AUTHORIZATION,
    REFERER, TRANSFER_ENCODING, WWW_AUTHENTICATE,
};
use spire_core::http::http::{HeaderName, HeaderValue, Method, StatusCode};
use spire_core::{Error, ErrorKind, Result};

use crate::client::{ReqwestSession, classify};
use crate::conversion::request_to_reqwest;

impl ReqwestSession {
    pub(crate) async fn send(
        &self,
        request: &Request,
    ) -> Result<(reqwest::Response, Option<Transfer>)> {
        let uri = request.uri().clone();
        let failed = |error: Error| error.with_uri(uri.clone());
        let mut converted = request_to_reqwest(request).map_err(failed)?;
        let mut hops = 0;
        let mut stripped_credentials = false;
        loop {
            let permit = if let Some(redirects) = request.extensions().get::<Redirects>() {
                let mut target_request = request.clone();
                // Keep the source URI that already passed crawler admission.
                // URL parsing may normalize its port or path; that is not a
                // new hop and must not charge a second host delay.
                if hops != 0 {
                    *target_request.uri_mut() =
                        converted.url().as_str().parse().map_err(|error| {
                            Error::invalid("invalid redirect URI").with_source(error)
                        })?;
                }
                *target_request.method_mut() = converted.method().clone();
                *target_request.headers_mut() = converted.headers().clone();
                *target_request.body_mut() = converted
                    .body()
                    .and_then(reqwest::Body::as_bytes)
                    .unwrap_or_default()
                    .to_vec()
                    .into();
                Some(redirects.check(&target_request).await?)
            } else {
                None
            };
            let previous = converted
                .try_clone()
                .ok_or_else(|| Error::invalid("request body cannot be replayed"))?;
            let response = self
                .client
                .execute(converted)
                .await
                .map_err(|error| failed(classify(error)))?;
            if !matches!(
                response.status(),
                StatusCode::MOVED_PERMANENTLY
                    | StatusCode::FOUND
                    | StatusCode::SEE_OTHER
                    | StatusCode::TEMPORARY_REDIRECT
                    | StatusCode::PERMANENT_REDIRECT
            ) {
                return Ok((response, permit));
            }
            let Some(location) = response.headers().get(LOCATION) else {
                return Ok((response, permit));
            };
            let location = location
                .to_str()
                .map_err(|error| Error::invalid("invalid redirect location").with_source(error))?;
            let mut target = response
                .url()
                .join(location)
                .map_err(|error| Error::invalid("invalid redirect URL").with_source(error))?;
            target.set_fragment(None);
            if !matches!(target.scheme(), "http" | "https")
                || !target.username().is_empty()
                || target.password().is_some()
            {
                return Err(Error::new(ErrorKind::Rejected)
                    .with_message("unsupported redirect URL")
                    .with_uri(uri));
            }
            hops += 1;
            if hops > 10 {
                return Err(Error::new(ErrorKind::Rejected)
                    .with_message("too many redirects")
                    .with_uri(uri));
            }
            converted = self.redirect_request(
                previous,
                response.status(),
                target.clone(),
                &mut stripped_credentials,
            )?;
        }
    }

    #[cfg_attr(
        not(feature = "cookies"),
        expect(
            clippy::unused_self,
            reason = "cookie-enabled builds consult this session's jar"
        )
    )]
    fn redirect_request(
        &self,
        mut converted: reqwest::Request,
        status: StatusCode,
        target: reqwest::Url,
        stripped_credentials: &mut bool,
    ) -> Result<reqwest::Request> {
        if status == StatusCode::SEE_OTHER && converted.method() != Method::HEAD
            || matches!(status, StatusCode::MOVED_PERMANENTLY | StatusCode::FOUND)
                && converted.method() == Method::POST
        {
            *converted.method_mut() = Method::GET;
            *converted.body_mut() = None;
            // Absent headers would be restored by reqwest's client defaults.
            // An empty body makes the transport remove Transfer-Encoding;
            // explicitly zero Content-Length to suppress stale framing.
            converted
                .headers_mut()
                .insert(CONTENT_LENGTH, HeaderValue::from_static("0"));
            for header in [CONTENT_TYPE, TRANSFER_ENCODING] {
                converted
                    .headers_mut()
                    .insert(header, HeaderValue::from_static(""));
            }
        }
        if target.origin() != converted.url().origin() {
            *stripped_credentials = true;
            for header in [
                AUTHORIZATION,
                PROXY_AUTHORIZATION,
                REFERER,
                WWW_AUTHENTICATE,
                HeaderName::from_static("cookie2"),
            ] {
                // An absent header would be restored from client defaults
                // on the next execute. An empty value suppresses those
                // defaults as well as credentials on the original seed.
                converted
                    .headers_mut()
                    .insert(header, HeaderValue::from_static(""));
            }
        }
        if *stripped_credentials {
            // Suppress source/default cookies, while still consulting the
            // destination's jar on every subsequent hop.
            #[cfg(feature = "cookies")]
            let cookie = {
                use reqwest::cookie::CookieStore;
                self.client
                    .cookies
                    .cookies(&target)
                    .unwrap_or_else(|| HeaderValue::from_static(""))
            };
            #[cfg(not(feature = "cookies"))]
            let cookie = HeaderValue::from_static("");
            converted.headers_mut().insert(COOKIE, cookie);
        }
        if *stripped_credentials {
            let target_uri: spire_core::http::http::Uri = target
                .as_str()
                .parse()
                .map_err(|error| Error::invalid("invalid redirect URI").with_source(error))?;
            let host = target_uri
                .authority()
                .ok_or_else(|| Error::invalid("redirect URL has no authority"))?;
            converted.headers_mut().insert(
                HOST,
                HeaderValue::from_str(host.as_str())
                    .map_err(|error| Error::invalid("invalid redirect host").with_source(error))?,
            );
        }
        *converted.url_mut() = target;
        Ok(converted)
    }
}
