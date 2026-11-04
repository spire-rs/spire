use spire::extract::FromRef;

#[derive(FromRef)]
union App {
    pool: u8,
}

fn main() {}
