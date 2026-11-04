use spire::extract::html::Select;

#[derive(Select)]
struct Product {
    #[select("h2")]
    name: String,
    /// Keeps the field name as the location, past its docs.
    price: String,
}

fn main() {}
