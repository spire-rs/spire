use spire::extract::html::Select;

#[derive(Select)]
#[select("article", attr = "id")]
struct Product {
    #[select("h2")]
    name: String,
}

fn main() {}
