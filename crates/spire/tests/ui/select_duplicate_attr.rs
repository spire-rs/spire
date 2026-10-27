use spire::extract::html::Select;

#[derive(Select)]
struct Product {
    #[select("h2")]
    #[select("h3")]
    name: String,
}

fn main() {}
