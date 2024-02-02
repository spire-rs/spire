use std::ffi::OsString;
use std::fmt;
use std::net::SocketAddr;

use crate::{Handler, Process, Result};

// TODO.
pub struct EdgeDriver {
    handler: Handler,
}

impl EdgeDriver {
    pub fn new() -> Self {
        let args = Vec::default();
        let exec = OsString::from("msedgedriver");
        let handler = Handler::new(&exec, &args);
        Self { handler }
    }
}
impl Default for EdgeDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Process for EdgeDriver {
    async fn run(&self) -> Result<()> {
        self.handler.run().await
    }

    async fn addr(&self) -> Result<SocketAddr> {
        todo!()
    }

    async fn close(self) -> Result<()> {
        self.handler.close().await
    }
}

impl fmt::Debug for EdgeDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EdgeDriver").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn run() -> Result<()> {
        let driver: EdgeDriver = EdgeDriver::new();
        driver.run().await?;
        let _ = driver.addr().await?;
        driver.close().await
    }
}
