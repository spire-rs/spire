use std::collections::BTreeMap;
use std::sync::LazyLock;

use derive_more::{Deref, DerefMut};
use scraper::Selector;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::backend::Session;
use crate::extract::FromContext;
use crate::extract::html::Html;
use crate::{Context, Error, Result};

fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("valid selector")
}

static JSON_LD: LazyLock<Selector> =
    LazyLock::new(|| selector(r#"script[type="application/ld+json"]"#));
static DESCRIPTION: LazyLock<Selector> =
    LazyLock::new(|| selector(r#"meta[name="description" i][content]"#));
static ROBOTS: LazyLock<Selector> = LazyLock::new(|| selector(r#"meta[name="robots" i][content]"#));
static CANONICAL: LazyLock<Selector> =
    LazyLock::new(|| selector(r#"link[rel="canonical" i][href]"#));
static LANGUAGE: LazyLock<Selector> = LazyLock::new(|| selector("html[lang]"));
static PROPERTIES: LazyLock<Selector> = LazyLock::new(|| selector("meta[property][content]"));
static TWITTER: LazyLock<Selector> =
    LazyLock::new(|| selector(r#"meta[name^="twitter:" i][content]"#));

impl Html {
    /// Returns the page's metadata, see [`Meta`].
    #[must_use]
    pub fn meta(&self) -> Meta {
        let first = |selector: &Selector, attr: &str| {
            self.0
                .select(selector)
                .find_map(|element| element.value().attr(attr))
                .map(|value| value.trim().to_owned())
        };
        let all = |selector: &Selector, key: &str| {
            let mut values = BTreeMap::new();
            for element in self.0.select(selector) {
                let element = element.value();
                if let (Some(key), Some(content)) = (element.attr(key), element.attr("content")) {
                    let key = key.trim().to_ascii_lowercase();
                    values
                        .entry(key)
                        .or_insert_with(|| content.trim().to_owned());
                }
            }
            values
        };

        Meta {
            title: self.title(),
            description: first(&DESCRIPTION, "content"),
            canonical: first(&CANONICAL, "href"),
            language: first(&LANGUAGE, "lang"),
            robots: first(&ROBOTS, "content"),
            properties: all(&PROPERTIES, "property"),
            twitter: all(&TWITTER, "name"),
        }
    }

    /// Returns every JSON-LD node on the page that deserializes as `T`, see
    /// [`JsonLd`].
    #[must_use]
    pub fn json_ld<T: DeserializeOwned>(&self) -> Vec<T> {
        let mut nodes = Vec::new();
        for script in self.0.select(&JSON_LD) {
            let text: String = script.text().collect();
            // Broken JSON-LD is common, and one bad block hides no others.
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                flatten(value, &mut nodes);
            }
        }
        nodes
            .into_iter()
            .filter_map(|node| serde_json::from_value(node).ok())
            .collect()
    }
}

/// Collects the top-level nodes of a JSON-LD block: the elements of an
/// array, the nodes of an `@graph`, or the block itself.
fn flatten(value: Value, nodes: &mut Vec<Value>) {
    match value {
        Value::Array(values) => {
            for value in values {
                flatten(value, nodes);
            }
        }
        Value::Object(mut object) => match object.remove("@graph") {
            Some(graph) => flatten(graph, nodes),
            None => nodes.push(Value::Object(object)),
        },
        _ => {}
    }
}

/// Extracts a page's metadata from its `<head>`, without selectors.
///
/// Values are trimmed and as written: the canonical URL may be relative.
/// Keys of [`properties`](Self::properties) and [`twitter`](Self::twitter)
/// are lowercase, and the first of repeated keys is kept. With an [`Html`]
/// at hand, [`Html::meta`] reads the same without parsing the page again.
///
/// ```
/// use spire::extract::html::Meta;
///
/// async fn article(meta: Meta) {
///     let title = meta.properties.get("og:title").or(meta.title.as_ref());
///     println!("{title:?}: {:?}", meta.description);
///     if meta.robots.as_deref().is_some_and(|robots| robots.contains("noindex")) {
///         return;
///     }
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Meta {
    /// The text of the `<title>`.
    pub title: Option<String>,
    /// `<meta name="description">`.
    pub description: Option<String>,
    /// `<link rel="canonical">`'s `href`.
    pub canonical: Option<String>,
    /// `<html lang>`.
    pub language: Option<String>,
    /// `<meta name="robots">`, such as `noindex, nofollow`.
    pub robots: Option<String>,
    /// Every `<meta property>`: Open Graph (`og:title`, `og:image`, ...) and
    /// its namespaces, such as `article:published_time`.
    pub properties: BTreeMap<String, String>,
    /// Every `<meta name="twitter:...">` card field.
    pub twitter: BTreeMap<String, String>,
}

impl<C, S> FromContext<C, S> for Meta
where
    C: Session,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: &Context<C>, state: &S) -> Result<Self> {
        Ok(Html::from_context(cx, state).await?.meta())
    }
}

/// Extracts typed nodes from the page's JSON-LD structured data.
///
/// Returns every node that deserializes as `T`. Many product, article and
/// event pages describe themselves with [schema.org](https://schema.org) data
/// in `<script type="application/ld+json">` blocks. Every block is read, an
/// array or `@graph` counting as its nodes, and blocks that are not valid
/// JSON are skipped. Deserialize into an enum tagged by `@type` to keep
/// only the types you want. With an [`Html`] at hand, [`Html::json_ld`]
/// reads the same without parsing the page again.
///
/// ```
/// use serde::Deserialize;
/// use spire::extract::html::JsonLd;
///
/// #[derive(Deserialize)]
/// #[serde(tag = "@type")]
/// enum Node {
///     Product { name: String, sku: Option<String> },
///     #[serde(other)]
///     Other,
/// }
///
/// async fn product(JsonLd(nodes): JsonLd<Node>) {
///     for node in nodes {
///         if let Node::Product { name, sku } = node {
///             println!("{name} ({sku:?})");
///         }
///     }
/// }
/// ```
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct JsonLd<T = Value>(pub Vec<T>);

impl<C, S, T> FromContext<C, S> for JsonLd<T>
where
    C: Session,
    S: Sync,
    T: DeserializeOwned,
{
    type Rejection = Error;

    async fn from_context(cx: &Context<C>, state: &S) -> Result<Self> {
        Ok(Self(Html::from_context(cx, state).await?.json_ld()))
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    const PAGE: &str = r#"<!doctype html><html lang="en-GB"><head>
        <title> Lamp </title>
        <meta name="Description" content=" A desk lamp. ">
        <meta name="robots" content="noindex, follow">
        <link rel="canonical" href="/products/lamp">
        <meta property="og:title" content="The lamp">
        <meta property="og:image" content="/a.png">
        <meta property="og:image" content="/b.png">
        <meta property="article:author" content="Ann">
        <meta name="twitter:card" content="summary">
        <script type="application/ld+json">{"@type": "Product", "name": "Lamp", "sku": "L1"}</script>
        <script type="application/ld+json">[{"@type": "BreadcrumbList"}, {"@type": "Product", "name": "Shade"}]</script>
        <script type="application/ld+json">{"@context": "https://schema.org", "@graph": [{"@type": "Organization", "name": "Shop"}]}</script>
        <script type="application/ld+json">{ not json </script>
        </head><body></body></html>"#;

    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(tag = "@type")]
    enum Node {
        Product {
            name: String,
            sku: Option<String>,
        },
        Organization {
            name: String,
        },
        #[serde(other)]
        Other,
    }

    #[test]
    fn reads_the_head() {
        let meta = Html::parse(PAGE).meta();
        assert_eq!(meta.title.as_deref(), Some("Lamp"));
        assert_eq!(meta.description.as_deref(), Some("A desk lamp."));
        assert_eq!(meta.canonical.as_deref(), Some("/products/lamp"));
        assert_eq!(meta.language.as_deref(), Some("en-GB"));
        assert_eq!(meta.robots.as_deref(), Some("noindex, follow"));
        assert_eq!(meta.properties["og:title"], "The lamp");
        assert_eq!(meta.properties["og:image"], "/a.png", "the first is kept");
        assert_eq!(meta.properties["article:author"], "Ann");
        assert_eq!(meta.twitter["twitter:card"], "summary");
        assert_eq!(
            Html::parse("<p>no head</p>").meta(),
            Meta {
                title: None,
                ..Meta::default()
            }
        );
    }

    #[test]
    fn reads_every_json_ld_node() {
        let nodes: Vec<Node> = Html::parse(PAGE).json_ld();
        assert_eq!(
            nodes,
            [
                Node::Product {
                    name: "Lamp".into(),
                    sku: Some("L1".into())
                },
                Node::Other,
                Node::Product {
                    name: "Shade".into(),
                    sku: None
                },
                Node::Organization {
                    name: "Shop".into()
                },
            ]
        );
        // Without a type, every node is a JSON value.
        assert_eq!(Html::parse(PAGE).json_ld::<Value>().len(), 4);
    }
}
