use spire::extract::html::Select;

#[derive(Select)]
struct Product {
    #[select()]
    name: String,
}

fn main() {}
