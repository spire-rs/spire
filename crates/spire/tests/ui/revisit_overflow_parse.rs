use spire::crawler::Revisit;

#[derive(Revisit)]
enum Page {
    #[revisit("99999999999999999999s")]
    List,
}

fn main() {}
