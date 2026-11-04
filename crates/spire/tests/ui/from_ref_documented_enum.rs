use spire::extract::FromRef;

/// Keeps the keyword as the location, past docs and visibility.
#[derive(FromRef)]
pub enum App {
    Pool,
}

fn main() {}
