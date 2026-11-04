use spire::crawler::Revisit;

#[derive(Revisit)]
enum Page {
    #[revisit("999999999999999d")]
    List,
}

fn main() {}
