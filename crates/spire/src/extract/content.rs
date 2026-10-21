use derive_more::{Deref, DerefMut};
use encoding_rs::{Encoding, UTF_8};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;

use crate::backend::Client;
use crate::extract::FromContext;
use crate::request::http::HeaderMap;
use crate::request::http::header::CONTENT_TYPE;
use crate::{Context, Error, ErrorKind, Result};

/// Fetches the response and collects its body.
async fn fetch_body<C: Client>(cx: Context<C>) -> Result<(HeaderMap, bytes::Bytes)> {
    let (parts, body) = cx.fetch().await?.into_parts();
    let collected = body.collect().await.map_err(|error| {
        Error::new(ErrorKind::Http)
            .with_message("failed to read the response body")
            .with_source(error)
    })?;
    Ok((parts.headers, collected.to_bytes()))
}

/// Decodes `bytes` by the charset in the `Content-Type` header or a byte
/// order mark, defaulting to UTF-8. Malformed sequences become U+FFFD.
pub(crate) fn decode(headers: &HeaderMap, bytes: &[u8]) -> String {
    let encoding = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(charset)
        .and_then(|label| Encoding::for_label(label.as_bytes()))
        .unwrap_or(UTF_8);

    let (text, _, _) = encoding.decode(bytes);
    text.into_owned()
}

/// Returns the `charset` parameter of a `Content-Type` value.
fn charset(content_type: &str) -> Option<&str> {
    content_type.split(';').skip(1).find_map(|parameter| {
        let (name, value) = parameter.split_once('=')?;
        let name = name.trim();
        name.eq_ignore_ascii_case("charset")
            .then(|| value.trim().trim_matches('"'))
    })
}

/// Extracts the raw response body.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct Bytes(pub bytes::Bytes);

impl<C, S> FromContext<C, S> for Bytes
where
    C: Client,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: Context<C>, _: &S) -> Result<Self> {
        let (_, bytes) = fetch_body(cx).await?;
        Ok(Self(bytes))
    }
}

/// Extracts the response body as text, decoded by the charset in the
/// `Content-Type` header.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct Text(pub String);

impl<C, S> FromContext<C, S> for Text
where
    C: Client,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: Context<C>, _: &S) -> Result<Self> {
        let (headers, bytes) = fetch_body(cx).await?;
        Ok(Self(decode(&headers, &bytes)))
    }
}

/// Extracts the response body as JSON.
#[derive(Debug, Default, Clone, Copy, Deref, DerefMut)]
pub struct Json<T>(pub T);

impl<C, S, T> FromContext<C, S> for Json<T>
where
    C: Client,
    S: Sync,
    T: DeserializeOwned,
{
    type Rejection = Error;

    async fn from_context(cx: Context<C>, _: &S) -> Result<Self> {
        let (_, bytes) = fetch_body(cx).await?;
        serde_json::from_slice(&bytes)
            .map(Self)
            .map_err(|error| Error::worker("failed to parse JSON").with_source(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::http::HeaderValue;

    fn headers(content_type: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
        headers
    }

    #[test]
    fn parses_charset_parameter() {
        assert_eq!(charset("text/html; charset=ISO-8859-1"), Some("ISO-8859-1"));
        assert_eq!(charset("text/html;Charset=\"utf-8\""), Some("utf-8"));
        assert_eq!(charset("text/html"), None);
    }

    #[test]
    fn decodes_by_charset() {
        let latin1 = [b'c', 0xE9];
        assert_eq!(
            decode(&headers("text/plain; charset=latin1"), &latin1),
            "c\u{e9}"
        );
        assert_eq!(
            decode(&HeaderMap::new(), "caf\u{e9}".as_bytes()),
            "caf\u{e9}"
        );
        assert_eq!(decode(&HeaderMap::new(), &[0xFF]), "\u{fffd}");
    }
}
