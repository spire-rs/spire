use spire::crawler::Tags;

#[derive(Tags)]
enum Page {
    List,
    #[tag(rename = "list")]
    Index,
}

fn main() {}
