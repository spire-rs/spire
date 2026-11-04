use spire::extract::FromRef;

#[derive(FromRef)]
struct App<T> {
    pool: T,
}

fn main() {}
