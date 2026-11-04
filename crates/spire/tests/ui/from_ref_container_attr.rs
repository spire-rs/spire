use spire::extract::FromRef;

#[derive(FromRef)]
#[from_ref(skip)]
struct App {
    pool: u8,
}

fn main() {}
