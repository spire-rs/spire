use spire::crawler::Revisit;

#[derive(Revisit)]
enum Page {
    #[revisit("1h")]
    #[revisit("2h")]
    List,
}

fn main() {}
