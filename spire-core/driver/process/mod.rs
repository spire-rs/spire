use std::net::SocketAddr;

pub use chromedriver::ChromeDriver;
pub use error::{Error, Result};
pub use geckodriver::{GeckoBuilder, GeckoDriver};
use handler::Handler;
pub use msedgedriver::MsEdgeDriver;
pub use safaridriver::SafariDriver;

mod chromedriver;
mod error;
mod geckodriver;
mod handler;
mod msedgedriver;
mod safaridriver;

// TODO: Download.
// TODO: Install.

// TODO: DriverProcess
#[async_trait::async_trait]
pub trait Driver: Sized {
    async fn run(&self) -> Result<()>;

    fn addr(&self) -> Result<SocketAddr>;

    async fn close(self) -> Result<()>;
}

pub enum Process {
    Chrome(ChromeDriver),
    Gecko(GeckoDriver),
    MsEdge(MsEdgeDriver),
    Safari(SafariDriver),
}

impl From<ChromeDriver> for Process {
    fn from(value: ChromeDriver) -> Self {
        Process::Chrome(value)
    }
}

impl From<GeckoDriver> for Process {
    fn from(value: GeckoDriver) -> Self {
        Process::Gecko(value)
    }
}

impl From<MsEdgeDriver> for Process {
    fn from(value: MsEdgeDriver) -> Self {
        Process::MsEdge(value)
    }
}

impl From<SafariDriver> for Process {
    fn from(value: SafariDriver) -> Self {
        Process::Safari(value)
    }
}

#[async_trait::async_trait]
impl Driver for Process {
    async fn run(&self) -> Result<()> {
        match self {
            Process::Chrome(x) => x.run().await,
            Process::Gecko(x) => x.run().await,
            Process::MsEdge(x) => x.run().await,
            Process::Safari(x) => x.run().await,
        }
    }

    fn addr(&self) -> Result<SocketAddr> {
        match self {
            Process::Chrome(x) => x.addr(),
            Process::Gecko(x) => x.addr(),
            Process::MsEdge(x) => x.addr(),
            Process::Safari(x) => x.addr(),
        }
    }

    async fn close(self) -> Result<()> {
        match self {
            Process::Chrome(x) => x.close().await,
            Process::Gecko(x) => x.close().await,
            Process::MsEdge(x) => x.close().await,
            Process::Safari(x) => x.close().await,
        }
    }
}
