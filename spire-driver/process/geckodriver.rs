use std::ffi::OsString;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::{Build, CommonSettings, DriverProcess};
use crate::{Handler, Result};

pub struct GeckoDriver {
    handler: Handler,
    addr: SocketAddr,
}

impl Build<GeckoDriver> for CommonSettings {
    fn build(self) -> GeckoDriver {
        let mut args = Vec::default();
        if let Some(port) = self.port {
            args.push("--port".into());
            args.push(port.to_string().into());
        }

        let exec = OsString::from("geckodriver");
        let handler = Handler::new(&exec, &args);

        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let port = self.port.unwrap_or(4444);
        let addr = SocketAddr::new(ip, port);

        GeckoDriver { handler, addr }
    }
}

#[async_trait::async_trait]
impl DriverProcess for GeckoDriver {
    type Builder = CommonSettings;

    async fn run(&self) -> Result<()> {
        self.handler.run().await
    }

    async fn addr(&self) -> Result<SocketAddr> {
        Ok(self.addr)
    }

    async fn close(self) -> Result<()> {
        self.handler.close().await
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
        let driver: GeckoDriver = GeckoDriver::builder().build();
        driver.run().await?;
        driver.close().await
    }

    #[tokio::test]
    async fn addr() -> Result<()> {
        let driver: GeckoDriver = GeckoDriver::builder().build();
        driver.run().await?;
        let _ = driver.addr().await?;
        driver.close().await
    }
}
