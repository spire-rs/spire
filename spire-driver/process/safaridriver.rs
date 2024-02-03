use std::ffi::OsString;
use std::fmt;
use std::net::SocketAddr;

use crate::{Driver, Handler, Result};

// TODO.
pub struct SafariDriver {
    handler: Handler,
}

impl SafariDriver {
    pub fn new() -> Self {
        let args = Vec::default();
        let exec = OsString::from("safaridriver");
        let handler = Handler::new(&exec, &args);
        Self { handler }
    }
}

impl Default for SafariDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Driver for SafariDriver {
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

impl fmt::Debug for SafariDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SafariDriver").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn run() -> Result<()> {
        let driver: SafariDriver = SafariDriver::new();
        driver.run().await?;
        let _ = driver.addr()?;
        driver.close().await
    }
}
