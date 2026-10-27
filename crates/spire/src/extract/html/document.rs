use std::fmt;
use std::sync::LazyLock;

use derive_more::{Deref, DerefMut};
use scraper::{ElementRef, Selector};

use super::select::{Select, private, select_in};
use crate::backend::Client;
use crate::extract::FromContext;
use crate::extract::body::Text;
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
/// use spire::extract::html::Html;
///
/// async fn handler(page: Html) {
///     println!("{:?}", page.title());
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

    /// Returns every `<a href>` with its element, in document order: the
    /// `href` as written in the page, and the element, e.g. to score a link
    /// by the markup around it with
    /// [`Link::with_priority`](crate::frontier::queue::Link::with_priority).
    /// [`links`](Self::links) gives them resolved, with their `rel` and
    /// text.
    pub fn anchors(&self) -> impl Iterator<Item = (&str, ElementRef<'_>)> {
        self.0
            .select(&LINKS)
            .filter_map(|element| Some((element.value().attr("href")?, element)))
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

    async fn from_context(cx: &Context<C>, state: &S) -> Result<Self> {
        let Text(text) = Text::from_context(cx, state).await?;
        Ok(Self::parse(&text))
    }
}

#[cfg(test)]
mod tests {
    use scraper::ElementRef;

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
    }

    #[test]
    fn anchors_pair_links_with_their_elements() {
        let html = Html::parse(PAGE);
        let anchors: Vec<_> = html
            .anchors()
            .map(|(href, anchor)| (href, anchor.value().name()))
            .collect();
        assert_eq!(anchors, [("/lamp", "a"), ("/chair", "a")]);
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
