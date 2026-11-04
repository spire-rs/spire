use spire::crawler::Revisit;

#[derive(Revisit)]
enum Page {
    #[revisit("1h30")]
    List,
}

fn main() {}
