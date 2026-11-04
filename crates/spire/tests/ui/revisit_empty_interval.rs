use spire::crawler::Revisit;

#[derive(Revisit)]
enum Page {
    #[revisit("")]
    List,
}

fn main() {}
