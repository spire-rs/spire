use spire::crawler::Revisit;

#[derive(Revisit)]
enum Page {
    #[revisit("1h")]
    List(u8),
}

fn main() {}
