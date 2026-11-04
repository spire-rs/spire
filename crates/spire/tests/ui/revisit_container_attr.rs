use spire::crawler::Revisit;

#[derive(Revisit)]
#[revisit("1h")]
enum Page {
    List,
}

fn main() {}
