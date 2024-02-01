use std::ffi::OsString;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::{Build, DriverProcess};
use crate::{Handler, Result};

pub struct GeckoDriver(Handler);

#[derive(Debug, Default)]
pub struct GeckoBuilder {}

impl Build<GeckoDriver> for GeckoBuilder {
    fn build(self) -> GeckoDriver {
        let executable = OsString::from("geckodriver");
        let handler = Handler::new(executable.as_ref(), &[]);
        GeckoDriver(handler)
    }
}

#[async_trait::async_trait]
impl DriverProcess for GeckoDriver {
    type Builder = GeckoBuilder;

    async fn run(&self) -> Result<()> {
        self.0.run().await
    }

    async fn addr(&self) -> Result<SocketAddr> {
        // TODO: Update from the first message.
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        Ok(SocketAddr::new(ip, 4444))
    }

    async fn close(self) -> Result<()> {
        self.0.close().await
    }
}

impl fmt::Debug for GeckoDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GeckoDriver").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn run() -> Result<()> {
        let driver = GeckoDriver::builder().build();
        driver.run().await?;
        driver.close().await
    }

    #[tokio::test]
    async fn addr() -> Result<()> {
        let driver = GeckoDriver::builder().build();
        driver.run().await?;
        let _ = driver.addr().await?;
        driver.close().await
    }
}
