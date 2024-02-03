use std::ffi::OsString;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::{Driver, Handler, Result};

pub struct GeckoDriver {
    handler: Handler,
    addr: SocketAddr,
}

#[derive(Debug, Default, Clone)]
pub struct GeckoBuilder {
    port: Option<u16>,
}

impl GeckoBuilder {
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn build(self) -> GeckoDriver {
        GeckoDriver::new(self)
    }
}

impl GeckoDriver {
    pub fn new(settings: GeckoBuilder) -> Self {
        let mut args = Vec::default();
        if let Some(port) = settings.port {
            args.push("--port".into());
            args.push(port.to_string().into());
        }

        let exec = OsString::from("geckodriver");
        let handler = Handler::new(&exec, &args);

        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let port = settings.port.unwrap_or(4444);
        let addr = SocketAddr::new(ip, port);

        Self { handler, addr }
    }

    pub fn builder() -> GeckoBuilder {
        GeckoBuilder::default()
    }
}

impl Default for GeckoDriver {
    fn default() -> Self {
        Self::builder().build()
    }
}

#[async_trait::async_trait]
impl Driver for GeckoDriver {
    async fn run(&self) -> Result<()> {
        self.handler.run().await
    }

    fn addr(&self) -> Result<SocketAddr> {
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
        let driver: GeckoDriver = GeckoDriver::default();
        driver.run().await?;
        let _ = driver.addr()?;
        driver.close().await
    }
}
