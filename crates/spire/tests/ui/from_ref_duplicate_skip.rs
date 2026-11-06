use spire::extract::FromRef;

#[derive(Clone, FromRef)]
struct App {
    #[from_ref(skip, skip)]
    pool: u8,
}

fn main() {}
