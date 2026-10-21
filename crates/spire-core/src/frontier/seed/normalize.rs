//! URL normalization for deduplication.

use std::fmt::Write as _;

use http::Uri;
use url::Url;

/// Query parameters that only track where a visitor came from.
const TRACKING: &[&str] = &[
    "fbclid", "gclid", "dclid", "gbraid", "wbraid", "msclkid", "yclid", "igshid", "mc_cid",
    "mc_eid", "_ga", "_gl", "ref_src",
];

fn is_tracking(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.starts_with("utm_") || TRACKING.contains(&key.as_str())
}

/// Returns the canonical form of `uri`, which URIs naming the same resource
/// share:
///
/// - scheme and host lowercase, default ports removed, `.` and `..` segments
///   resolved, and an empty path made `/`;
/// - percent-escapes uppercase, and escaped unreserved characters decoded;
/// - query parameters sorted, empty queries and tracking parameters such as
///   `utm_*` or `fbclid` removed;
/// - the fragment removed.
pub(crate) fn canonical(uri: &Uri) -> String {
    let Ok(mut url) = Url::parse(&uri.to_string()) else {
        return uri.to_string();
    };
    url.set_fragment(None);

    let path = normalize_escapes(url.path());
    url.set_path(&path);

    let mut pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| !is_tracking(key))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    pairs.sort();
    if pairs.is_empty() {
        url.set_query(None);
    } else {
        url.query_pairs_mut().clear().extend_pairs(pairs);
    }

    url.into()
}

/// Uppercases percent-escapes and decodes those of unreserved characters.
fn normalize_escapes(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut normalized = String::with_capacity(path.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = (bytes[i] == b'%')
            .then(|| path.get(i + 1..i + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());

        match escaped {
            Some(byte) if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) => {
                normalized.push(char::from(byte));
                i += 3;
            }
            Some(byte) => {
                let _ = write!(normalized, "%{byte:02X}");
                i += 3;
            }
            None => {
                let ch = path[i..].chars().next().unwrap_or_default();
                normalized.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canon(uri: &str) -> String {
        canonical(&uri.parse().unwrap())
    }

    #[test]
    fn normalizes_scheme_host_port_and_path() {
        assert_eq!(canon("HTTPS://Example.COM:443"), "https://example.com/");
        assert_eq!(
            canon("http://example.com:80/a/./b/../c"),
            "http://example.com/a/c"
        );
        assert_eq!(
            canon("https://example.com:8443/"),
            "https://example.com:8443/"
        );
    }

    #[test]
    fn normalizes_escapes() {
        assert_eq!(
            canon("https://example.com/%7euser/%2f"),
            "https://example.com/~user/%2F"
        );
        assert_eq!(
            canon("https://example.com/a%20b"),
            "https://example.com/a%20b"
        );
    }

    #[test]
    fn sorts_query_and_drops_tracking() {
        assert_eq!(
            canon("https://example.com/p?b=2&utm_source=x&a=1&FBCLID=y#top"),
            "https://example.com/p?a=1&b=2"
        );
        assert_eq!(
            canon("https://example.com/p?utm_medium=x"),
            "https://example.com/p"
        );
        assert_eq!(canon("https://example.com/p?"), "https://example.com/p");
    }
}
