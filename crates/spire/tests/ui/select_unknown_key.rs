use spire::extract::html::Select;

#[derive(Select)]
struct Product {
    #[select("a", href = "url")]
    link: String,
}

fn main() {}
