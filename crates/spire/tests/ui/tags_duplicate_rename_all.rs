use spire::crawler::Tags;

#[derive(Tags)]
#[tag(rename_all = "lowercase")]
enum Page {
    ItemList,
    Itemlist,
}

fn main() {}
