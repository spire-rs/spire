pub use label::Label;

// mod context;
mod context;
mod control;
mod label;
mod queue;

pub trait Agent {}

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

#[cfg(test)]
mod test {}
