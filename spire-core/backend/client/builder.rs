use crate::backend::HttpClient;

pub struct HttpClientBuilder {}

impl HttpClientBuilder {
    pub fn new() -> Self {
        todo!()
    }

    pub fn build(self) -> HttpClient {
        todo!()
    }
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}
