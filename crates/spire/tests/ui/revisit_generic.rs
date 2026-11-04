use spire::crawler::Revisit;

#[derive(Revisit)]
enum Page<T> {
    List,
    Other(std::marker::PhantomData<T>),
}

fn main() {}
