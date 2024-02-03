use std::ffi::OsString;
use std::fmt;
use std::net::SocketAddr;

use crate::{Driver, Handler, Result};

// TODO.
pub struct ChromeDriver {
    handler: Handler,
}

impl ChromeDriver {
    pub fn new() -> Self {
        let args = Vec::default();
        let exec = OsString::from("chromedriver");
        let handler = Handler::new(&exec, &args);
        Self { handler }
    }
}

impl Default for ChromeDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Driver for ChromeDriver {
    async fn run(&self) -> Result<()> {
        self.handler.run().await
    }

    fn addr(&self) -> Result<SocketAddr> {
        todo!()
    }

    async fn close(self) -> Result<()> {
        self.handler.close().await
    }
}

impl fmt::Debug for ChromeDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChromeDriver").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn run() -> Result<()> {
        let driver: ChromeDriver = ChromeDriver::new();
        driver.run().await?;
        let _ = driver.addr()?;
        driver.close().await
    }
}
