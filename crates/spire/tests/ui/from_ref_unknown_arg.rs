use spire::extract::FromRef;

#[derive(FromRef)]
struct App {
    #[from_ref(clone)]
    pool: u8,
}

fn main() {}
