use spire::crawler::Revisit;

#[derive(Revisit)]
enum Page {
    #[revisit("18446744073709551615s1s")]
    List,
}

fn main() {}
