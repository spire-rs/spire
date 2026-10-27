use spire::crawler::Tags;

#[derive(Tags)]
enum Page {
    List,
    Item { id: u32 },
}

fn main() {}
