use std::ffi::OsString;
use std::fmt;
use std::net::SocketAddr;

use crate::driver::process::{Driver, Handler, Result};

// TODO.
pub struct MsEdgeDriver {
    handler: Handler,
}

impl MsEdgeDriver {
    pub fn new() -> Self {
        let args = Vec::default();
        let exec = OsString::from("msedgedriver");
        let handler = Handler::new(&exec, &args);
        Self { handler }
    }
}
impl Default for MsEdgeDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Driver for MsEdgeDriver {
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

impl fmt::Debug for MsEdgeDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EdgeDriver").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn run() -> Result<()> {
        let driver: MsEdgeDriver = MsEdgeDriver::new();
        driver.run().await?;
        let _ = driver.addr()?;
        driver.close().await
    }
}
