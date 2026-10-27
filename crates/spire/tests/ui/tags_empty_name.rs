use spire::crawler::Tags;

#[derive(Tags)]
enum Page {
    List,
    #[tag(rename = "")]
    Item,
}

fn main() {}
