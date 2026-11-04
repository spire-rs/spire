use spire::crawler::Revisit;

#[derive(Revisit)]
enum Page {
    #[revisit("h")]
    List,
}

fn main() {}
