//! The serialized form of a [`Seed`].

use ::http::{HeaderValue, Method};
use bytes::Bytes;

use crate::crawler::{Attempt, Depth, Tag};
use crate::frontier::seed::Seed;

/// The serialized form of a [`Seed`]: `http` types are not serializable.
#[derive(serde::Serialize, serde::Deserialize)]
struct SeedRepr {
    method: String,
    uri: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    headers: Vec<(String, Vec<u8>)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    body: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
    #[serde(default)]
    depth: u32,
    #[serde(default)]
    priority: i32,
    #[serde(default)]
    attempt: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    identity: Option<String>,
}

/// Serializes the method and URI as strings, headers and body as bytes.
impl serde::Serialize for Seed {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let headers = self
            .headers()
            .iter()
            .map(|(name, value)| (name.as_str().to_owned(), value.as_bytes().to_vec()))
            .collect();

        let repr = SeedRepr {
            method: self.method.as_str().to_owned(),
            uri: self.uri.to_string(),
            headers,
            body: self.body().to_vec(),
            tag: self.tag.as_ref().map(|tag| tag.as_str().to_owned()),
            depth: self.depth.get(),
            priority: self.priority,
            attempt: self.attempt.get(),
            identity: self.identity().map(str::to_owned),
        };
        repr.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Seed {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;

        let repr = SeedRepr::deserialize(deserializer)?;
        let method = Method::from_bytes(repr.method.as_bytes()).map_err(D::Error::custom)?;
        let uri = repr.uri.parse().map_err(D::Error::custom)?;
        let mut seed = Self::new(method, uri).map_err(D::Error::custom)?;

        for (name, value) in repr.headers {
            let name = ::http::HeaderName::from_bytes(name.as_bytes()).map_err(D::Error::custom)?;
            let value = HeaderValue::from_bytes(&value).map_err(D::Error::custom)?;
            seed.extras_mut().headers.append(name, value);
        }
        if !repr.body.is_empty() {
            seed.extras_mut().body = Bytes::from(repr.body);
        }
        seed.tag = repr.tag.map(Tag::from);
        seed.depth = Depth::new(repr.depth);
        seed.priority = repr.priority;
        seed.attempt = Attempt::new(repr.attempt);
        if let Some(identity) = repr.identity {
            seed = seed.with_identity(identity);
        }
        Ok(seed)
    }
}

#[cfg(test)]
mod tests {
    use ::http::Uri;

    use super::*;

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip() {
        let seed = Seed::new(Method::POST, Uri::from_static("https://example.com/api"))
            .unwrap()
            .with_header(
                ::http::header::ACCEPT,
                HeaderValue::from_static("application/json"),
            )
            .with_body("{}")
            .with_tag("api")
            .with_depth(Depth::new(2))
            .with_priority(-1)
            .with_identity("account-a")
            .next_attempt();

        let json = serde_json::to_string(&seed).unwrap();
        let back: Seed = serde_json::from_str(&json).unwrap();
        assert_eq!(back.fingerprint(), seed.fingerprint());
        assert_eq!(back.tag(), seed.tag());
        assert_eq!(
            (back.depth(), back.priority(), back.attempt()),
            (Depth::new(2), -1, Attempt::new(1))
        );
        assert_eq!(back.headers()["accept"], "application/json");
        assert_eq!(back.identity(), Some("account-a"));

        let minimal: Seed =
            serde_json::from_str(r#"{"method":"GET","uri":"https://example.com/"}"#).unwrap();
        assert_eq!(minimal.depth(), Depth::ROOT);
        assert!(serde_json::from_str::<Seed>(r#"{"method":"GET","uri":"/relative"}"#).is_err());
    }
}
