use spire::crawler::Revisit;

#[derive(Revisit)]
enum Page {
    #[revisit("1w")]
    List,
}

fn main() {}
