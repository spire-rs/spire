use spire::extract::FromRef;

#[derive(Debug, Clone)]
pub struct AppState {
    links: u32,
}

impl AppState {
    pub fn new() -> Self {
        Self { links: 2 }
    }
}

impl FromRef<AppState> for u32 {
    fn from_ref(input: &AppState) -> Self {
        input.links
    }
}
