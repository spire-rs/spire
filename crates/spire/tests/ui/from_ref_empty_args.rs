use spire::extract::FromRef;

#[derive(FromRef)]
struct App {
    #[from_ref()]
    pool: u8,
}

fn main() {}
