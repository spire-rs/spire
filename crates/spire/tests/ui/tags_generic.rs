use spire::crawler::Tags;

#[derive(Tags)]
enum Page<T> {
    List,
    Item(T),
}

fn main() {}
