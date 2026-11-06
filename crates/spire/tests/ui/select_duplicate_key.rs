use spire::extract::html::Select;

#[derive(Select)]
struct Product {
    #[select("a", attr = "href", attr = "title")]
    link: String,
}

fn main() {}
