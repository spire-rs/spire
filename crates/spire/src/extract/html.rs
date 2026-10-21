use std::fmt;
use std::sync::LazyLock;

use derive_more::{Deref, DerefMut};
use scraper::{ElementRef, Selector};

use crate::backend::Client;
use crate::extract::FromContext;
use crate::extract::content::Text;
use crate::{Context, Error, Result};

/// Every link with an `href`.
static LINKS: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("a[href]").expect("valid selector"));

/// Every title.
static TITLE: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("title").expect("valid selector"));

/// Extracts the response body as a parsed HTML document.
///
/// The document is `Send` but not `Sync`: a handler can keep it across
/// `.await`, but not references into it, so collect what you need first.
///
/// ```
/// use spire::extract::Html;
/// use spire::frontier::queue::Queue;
///
/// async fn handler(queue: Queue, page: Html) -> spire::Result<()> {
///     println!("{:?}", page.title());
///     queue.follow_all(page.links()).await?;
///     Ok(())
/// }
/// ```
#[derive(Clone, Deref, DerefMut)]
pub struct Html(pub scraper::Html);

impl Html {
    /// Parses a document.
    #[must_use]
    pub fn parse(html: &str) -> Self {
        Self(scraper::Html::parse_document(html))
    }

    /// Returns the text of the `<title>`, if any.
    #[must_use]
    pub fn title(&self) -> Option<String> {
        self.0.select(&TITLE).next().map(private::text)
    }

    /// Returns the `href` of every link, as written in the page.
    ///
    /// Pass them to [`Queue::follow_all`](crate::frontier::queue::Queue::follow_all),
    /// which resolves relative links.
    #[must_use]
    pub fn links(&self) -> Vec<String> {
        self.0
            .select(&LINKS)
            .filter_map(|element| element.value().attr("href"))
            .map(str::to_owned)
            .collect()
    }

    /// Extracts a `T` from the document, see [`Select`]: the first match of
    /// its root selector, an `Option<T>` if it may be missing, or a `Vec<T>`
    /// for every match.
    ///
    /// # Errors
    ///
    /// Returns an error if nothing matches a required `T`'s root selector, or
    /// a field cannot be extracted.
    pub fn select<T: Select>(&self) -> Result<T> {
        select_in(self.0.root_element())
    }
}

impl fmt::Debug for Html {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Html")
            .field("title", &self.title())
            .finish_non_exhaustive()
    }
}

impl<C, S> FromContext<C, S> for Html
where
    C: Client,
    S: Sync,
{
    type Rejection = Error;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self> {
        let Text(text) = Text::from_context(cx, state).await?;
        Ok(Self::parse(&text))
    }
}

/// Extraction of a value from an HTML element, usually derived.
///
/// With the `macros` feature, `#[derive(Select)]` implements it from CSS
/// selectors on the fields. A field's type decides how many matches it
/// takes: `Option<T>` the first if any, `Vec<T>` all of them, and any other
/// `T` exactly the first. Values are the trimmed text of the element, or an
/// attribute with `attr = "..."`, parsed with [`FromStr`](std::str::FromStr).
///
/// ```
/// # #[cfg(feature = "macros")] {
/// use spire::extract::Select;
///
/// #[derive(Debug, Select)]
/// #[select("article.product")]
/// struct Product {
///     #[select("h2")]
///     name: String,
///     #[select(".price")]
///     price: Option<f64>,
///     #[select("a.tag")]
///     tags: Vec<String>,
///     #[select("a.more", attr = "href")]
///     link: String,
/// }
/// # }
/// ```
///
/// The container's `#[select("...")]` sets [`ROOT`](Self::ROOT): each match
/// becomes one value. Without it, the whole document is one value.
///
/// `Option<T>` and `Vec<T>` select too, taking the first match of `T` if any
/// and every match of `T`, so [`Element<Vec<T>>`](Element) extracts them
/// all.
pub trait Select: Sized {
    /// The CSS selector of the elements each value is extracted from, or
    /// `None` for the document root.
    const ROOT: Option<&'static str> = None;

    /// Extracts a value from `element`.
    ///
    /// # Errors
    ///
    /// Returns an error if a required field has no match or a value cannot
    /// be parsed.
    fn select(element: ElementRef<'_>) -> Result<Self>;
}

/// Extracts a `T` from `element`: from the first match of its root selector
/// below it, or from the element itself without one.
fn select_in<T: Select>(element: ElementRef<'_>) -> Result<T> {
    let Some(css) = T::ROOT else {
        return T::select(element);
    };
    let found = element.select(&private::selector(css)?).next();
    let found = found.ok_or_else(|| Error::worker(format!("no element matches `{css}`")))?;
    T::select(found)
}

/// The first match of `T`, if any.
impl<T: Select> Select for Option<T> {
    fn select(element: ElementRef<'_>) -> Result<Self> {
        let Some(css) = T::ROOT else {
            return T::select(element).map(Some);
        };
        let found = element.select(&private::selector(css)?).next();
        found.map(T::select).transpose()
    }
}

/// Every match of `T`.
impl<T: Select> Select for Vec<T> {
    fn select(element: ElementRef<'_>) -> Result<Self> {
        let Some(css) = T::ROOT else {
            return Ok(vec![T::select(element)?]);
        };
        let selector = private::selector(css)?;
        element.select(&selector).map(T::select).collect()
    }
}

/// Extracts a `T` from the response body, see [`Select`] and
/// [`Html::select`]: `Element<T>` for the first match, `Element<Option<T>>`
/// if it may be missing, and `Element<Vec<T>>` for every match.
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct Element<T>(pub T);

impl<C, S, T> FromContext<C, S> for Element<T>
where
    C: Client,
    S: Sync,
    T: Select + Send,
{
    type Rejection = Error;

    async fn from_context(cx: Context<C>, state: &S) -> Result<Self> {
        let html = Html::from_context(cx, state).await?;
        html.select().map(Self)
    }
}

/// Helpers for code generated by `#[derive(Select)]`; not public API.
#[doc(hidden)]
pub mod private {
    use std::str::FromStr;
    pub use std::sync::OnceLock;

    pub use scraper::{ElementRef, Selector};

    use crate::{Error, Result};

    /// Parses a CSS selector.
    pub fn selector(css: &str) -> Result<Selector> {
        Selector::parse(css)
            .map_err(|error| Error::invalid(format!("invalid CSS selector `{css}`: {error}")))
    }

    /// Parses a CSS selector once, caching it in `cell`.
    pub fn cached(cell: &'static OnceLock<Selector>, css: &str) -> Result<&'static Selector> {
        if let Some(selector) = cell.get() {
            return Ok(selector);
        }
        let selector = selector(css)?;
        Ok(cell.get_or_init(|| selector))
    }

    /// Returns the text of `element`, trimmed and with whitespace collapsed.
    #[must_use]
    pub fn text(element: ElementRef<'_>) -> String {
        let mut text = String::new();
        for word in element.text().flat_map(str::split_whitespace) {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(word);
        }
        text
    }

    /// Returns the values of the elements under `element` matching
    /// `selector`: their text, or `attr` when given. A missing selector
    /// takes `element` itself.
    #[must_use]
    pub fn values<'a>(
        element: ElementRef<'a>,
        selector: Option<&'a Selector>,
        attr: Option<&'static str>,
    ) -> Vec<String> {
        let value = |element: ElementRef<'_>| match attr {
            Some(attr) => element.value().attr(attr).map(str::to_owned),
            None => Some(text(element)),
        };

        match selector {
            Some(selector) => element.select(selector).filter_map(value).collect(),
            None => value(element).into_iter().collect(),
        }
    }

    /// Parses `value` for `field` of `owner`.
    pub fn parse<T: FromStr>(owner: &str, field: &str, value: &str) -> Result<T>
    where
        T::Err: std::fmt::Display,
    {
        value.parse().map_err(|error| {
            Error::worker(format!(
                "`{owner}.{field}`: cannot parse `{value}`: {error}"
            ))
        })
    }

    /// Reports a required `field` of `owner` that matched nothing.
    #[must_use]
    pub fn missing(owner: &str, field: &str, css: &str) -> Error {
        Error::worker(format!("`{owner}.{field}`: no element matches `{css}`"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"
        <html><head><title> Shop </title></head><body>
          <article class="product"><h2>Lamp</h2><span class="price">12.5</span>
            <a class="tag">home</a><a class="tag">light</a><a class="more" href="/lamp">more</a>
          </article>
          <article class="product"><h2>  Desk
            chair </h2><a class="more" href="/chair">more</a>
          </article>
        </body></html>
    "#;

    #[derive(Debug, PartialEq)]
    struct Product {
        name: String,
        price: Option<f64>,
        tags: Vec<String>,
        link: String,
    }

    /// What `#[derive(Select)]` generates for `Product`.
    impl Select for Product {
        const ROOT: Option<&'static str> = Some("article.product");

        fn select(element: ElementRef<'_>) -> Result<Self> {
            let name_css = private::selector("h2")?;
            let price_css = private::selector(".price")?;
            let tags_css = private::selector("a.tag")?;
            let link_css = private::selector("a.more")?;

            let name = private::values(element, Some(&name_css), None);
            let price = private::values(element, Some(&price_css), None);
            let tags = private::values(element, Some(&tags_css), None);
            let link = private::values(element, Some(&link_css), Some("href"));
            Ok(Self {
                name: match name.first() {
                    Some(value) => private::parse("Product", "name", value)?,
                    None => return Err(private::missing("Product", "name", "h2")),
                },
                price: price
                    .first()
                    .map(|value| private::parse("Product", "price", value))
                    .transpose()?,
                tags: tags
                    .iter()
                    .map(|value| private::parse("Product", "tags", value))
                    .collect::<Result<_>>()?,
                link: match link.first() {
                    Some(value) => private::parse("Product", "link", value)?,
                    None => return Err(private::missing("Product", "link", "a.more")),
                },
            })
        }
    }

    #[test]
    fn document_helpers() {
        let html = Html::parse(PAGE);
        assert_eq!(html.title().as_deref(), Some("Shop"));
        assert_eq!(html.links(), ["/lamp", "/chair"]);
    }

    #[test]
    fn selects_every_root_match() {
        let products: Vec<Product> = Html::parse(PAGE).select::<Vec<_>>().unwrap();
        assert_eq!(
            products,
            [
                Product {
                    name: "Lamp".to_owned(),
                    price: Some(12.5),
                    tags: vec!["home".to_owned(), "light".to_owned()],
                    link: "/lamp".to_owned(),
                },
                Product {
                    name: "Desk chair".to_owned(),
                    price: None,
                    tags: Vec::new(),
                    link: "/chair".to_owned(),
                },
            ]
        );

        let first: Product = Html::parse(PAGE).select().unwrap();
        assert_eq!(first.name, "Lamp");
    }

    #[test]
    fn reports_missing_and_unparsable_fields() {
        let missing = Html::parse(r#"<article class="product"><h2>x</h2></article>"#);
        let error = missing.select::<Product>().unwrap_err();
        assert_eq!(
            error.to_string(),
            "`Product.link`: no element matches `a.more`"
        );

        let bad_price = r#"<article class="product"><h2>x</h2><span class="price">free</span><a class="more" href="/">m</a></article>"#;
        let error = Html::parse(bad_price).select::<Product>().unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("`Product.price`: cannot parse `free`")
        );

        assert!(Html::parse("<p></p>").select::<Product>().is_err());
    }

    #[test]
    fn optional_and_repeated_selections() {
        let empty = Html::parse("<p></p>");
        assert_eq!(empty.select::<Option<Product>>().unwrap(), None);
        assert_eq!(empty.select::<Vec<Product>>().unwrap(), []);

        let first = Html::parse(PAGE).select::<Option<Product>>().unwrap();
        assert_eq!(first.map(|product| product.name).as_deref(), Some("Lamp"));
    }
}
