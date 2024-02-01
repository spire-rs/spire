use std::fmt;
use std::net::SocketAddr;

use crate::{Build, CommonSettings, DriverProcess};
use crate::{Handler, Result};

pub struct ChromeDriver(Handler);

impl Build<ChromeDriver> for CommonSettings {
    fn build(self) -> ChromeDriver {
        todo!()
    }
}

#[async_trait::async_trait]
impl DriverProcess for ChromeDriver {
    type Builder = CommonSettings;

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
        let driver: ChromeDriver = ChromeDriver::builder().build();
        driver.run().await?;
        driver.close().await
    }

    #[tokio::test]
    async fn addr() -> Result<()> {
        let driver: ChromeDriver = ChromeDriver::builder().build();
        driver.run().await?;
        let _ = driver.addr().await?;
        driver.close().await
    }
}
