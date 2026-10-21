//! `#[derive(Select)]` as users write it.

#![cfg(feature = "macros")]

use spire::extract::{Html, Select};

const PAGE: &str = r#"
    <html><body>
      <h1 class="shop" data-id="7">  The   Shop </h1>
      <article class="product"><h2>Lamp</h2><span class="price">12.5</span>
        <a class="tag">home</a><a class="tag">light</a><a class="more" href="/lamp">more</a>
      </article>
      <article class="product"><h2>Chair</h2><a class="more" href="/chair">more</a></article>
    </body></html>
"#;

#[derive(Debug, PartialEq, Select)]
#[select("article.product")]
struct Product {
    #[select("h2")]
    name: String,
    #[select(".price")]
    price: Option<f64>,
    #[select("a.tag")]
    tags: Vec<String>,
    #[select("a.more", attr = "href")]
    link: String,
}

#[derive(Debug, Select)]
struct Shop {
    #[select("h1.shop")]
    title: String,
    #[select("h1.shop", attr = "data-id")]
    id: u32,
    #[select("article.product h2")]
    names: Vec<String>,
}

#[derive(Debug, Select)]
#[select("a.more")]
struct MoreLink {
    #[select(attr = "href")]
    href: String,
}

#[test]
fn derives_container_and_field_selectors() {
    let products: Vec<Product> = Html::parse(PAGE).select_all().unwrap();
    assert_eq!(products.len(), 2);
    assert_eq!(products[0].price, Some(12.5));
    assert_eq!(products[0].tags, ["home", "light"]);
    assert_eq!(products[1].link, "/chair");
    assert_eq!(products[1].price, None);
}

#[test]
fn derives_document_level_values() {
    let shop: Shop = Html::parse(PAGE).select_one().unwrap();
    assert_eq!(shop.title, "The Shop");
    assert_eq!(shop.id, 7);
    assert_eq!(shop.names, ["Lamp", "Chair"]);
}

#[test]
fn derives_attributes_of_the_root_element() {
    let links: Vec<MoreLink> = Html::parse(PAGE).select_all().unwrap();
    let hrefs: Vec<_> = links.into_iter().map(|link| link.href).collect();
    assert_eq!(hrefs, ["/lamp", "/chair"]);
}

#[test]
fn reports_missing_fields() {
    let error = Html::parse("<p>empty</p>")
        .select_one::<Shop>()
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "`Shop.title`: no element matches `h1.shop`"
    );
}
