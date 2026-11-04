use spire::crawler::Revisit;

#[derive(Revisit)]
enum Page {
    #[revisit("0s")]
    List,
}

fn main() {}
