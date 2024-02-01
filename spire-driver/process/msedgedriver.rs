use std::fmt;
use std::net::SocketAddr;

use crate::{Build, DriverProcess};
use crate::{Handler, Result};

pub struct EdgeDriver(Handler);

#[derive(Debug, Default)]
pub struct EdgeBuilder {}

impl Build<EdgeDriver> for EdgeBuilder {
    fn build(self) -> EdgeDriver {
        todo!()
    }
}

#[async_trait::async_trait]
impl DriverProcess for EdgeDriver {
    type Builder = EdgeBuilder;

    async fn run(&self) -> Result<()> {
        self.0.run().await
    }

    async fn addr(&self) -> Result<SocketAddr> {
        todo!()
    }

    async fn close(self) -> Result<()> {
        self.0.close().await
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
        let driver = EdgeDriver::builder().build();
        driver.run().await?;
        driver.close().await
    }

    #[tokio::test]
    async fn addr() -> Result<()> {
        let driver = EdgeDriver::builder().build();
        driver.run().await?;
        let _ = driver.addr().await?;
        driver.close().await
    }
}
