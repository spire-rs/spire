pub use context::HandlerContext;

mod context;
mod control;

#[derive(Debug, Default, Clone)]
pub struct Builder {}

impl Builder {
    pub fn build(self) -> Collector {
        todo!()
    }
}

#[derive(Debug)]
pub struct Collector {}

impl Collector {
    pub fn new() -> Self {
        todo!()
    }

    pub fn builder() -> Builder {
        Builder::default()
    }
}

impl Default for Collector {
    fn default() -> Self {
        todo!()
    }
}
