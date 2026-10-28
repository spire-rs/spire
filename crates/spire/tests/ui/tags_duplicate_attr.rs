use spire::crawler::Tags;

#[derive(Tags)]
enum Page {
    #[tag(rename = "index")]
    #[tag(rename = "list")]
    List,
}

fn main() {}
