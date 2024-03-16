use crate::backend::HttpClientPool;

pub struct HttpClientBuilder {}

impl HttpClientBuilder {
    pub fn new() -> Self {
        todo!()
    }

    pub fn build(self) -> HttpClientPool {
        todo!()
    }
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
