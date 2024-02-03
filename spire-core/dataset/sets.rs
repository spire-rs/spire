use crate::dataset::BoxDataset;

pub struct Datasets {
    // datasets: HashMap<TypeId, BoxDataset<dyn Any + Send>>,
}

impl Datasets {
    pub fn new() -> Self {
        todo!()
    }

    pub fn dataset<T>(&self) -> BoxDataset<T> {
        todo!()
    }
}

impl Default for Datasets {
    fn default() -> Self {
        Self::new()
    }
}
