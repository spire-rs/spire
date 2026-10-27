use spire::extract::html::Select;

#[derive(Select)]
struct Product {
    #[select("h2")]
    name: String,
    price: String,
}

fn main() {}
