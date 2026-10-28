use spire::extract::html::Select;

#[derive(Select)]
struct Product(#[select("h2")] String);

fn main() {}
