use serde_json::{Value, json};
use spire_core::backend::document::{DocumentLink, DocumentLinks, LinkKind, Visibility};
use spire_core::{Error, Result};
use thirtyfour::WebDriver;

use crate::error::from_webdriver;

pub(crate) async fn capture(driver: &WebDriver, selector: Option<&str>) -> Result<DocumentLinks> {
    let result = driver
        .execute(include_str!("capture.js"), vec![json!(selector)])
        .await
        .map_err(from_webdriver)?;
    parse(result.json())
}

fn parse(value: &Value) -> Result<DocumentLinks> {
    if let Some(error) = value.get("error").and_then(Value::as_str) {
        return Err(Error::invalid(format!("invalid link selector: {error}")));
    }
    let text = |value: &Value, name| {
        value
            .get(name)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| Error::backend("invalid document link capture"))
    };
    let links = value
        .get("links")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::backend("invalid document link capture"))?
        .iter()
        .map(|link| {
            Ok(DocumentLink {
                href: text(link, "href")?,
                text: text(link, "text")?,
                rel: link
                    .get("rel")
                    .and_then(Value::as_array)
                    .ok_or_else(|| Error::backend("invalid document link capture"))?
                    .iter()
                    .map(|token| {
                        token
                            .as_str()
                            .map(str::to_owned)
                            .ok_or_else(|| Error::backend("invalid document link capture"))
                    })
                    .collect::<Result<Vec<_>>>()?,
                kind: match text(link, "kind")?.as_str() {
                    "a" => LinkKind::Anchor,
                    "area" => LinkKind::Area,
                    _ => return Err(Error::backend("invalid document link kind")),
                },
                visibility: match text(link, "visibility")?.as_str() {
                    "visible" => Visibility::Visible,
                    "hidden" => Visibility::Hidden,
                    "unknown" => Visibility::Unknown,
                    _ => return Err(Error::backend("invalid document link visibility")),
                },
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(DocumentLinks {
        uri: text(value, "uri")?
            .parse()
            .map_err(|error| Error::backend("invalid document URI").with_source(error))?,
        base_uri: text(value, "base_uri")?,
        links,
    })
}
