use std::net::SocketAddr;

#[cfg(feature = "chromedriver")]
pub use chromedriver::ChromeDriver;
#[cfg(feature = "geckodriver")]
pub use geckodriver::{GeckoBuilder, GeckoDriver};
#[cfg(feature = "msedgedriver")]
pub use msedgedriver::MsEdgeDriver;
#[cfg(feature = "safaridriver")]
pub use safaridriver::SafariDriver;

use crate::Result;

#[cfg(feature = "chromedriver")]
mod chromedriver;
#[cfg(feature = "geckodriver")]
mod geckodriver;
#[cfg(feature = "msedgedriver")]
mod msedgedriver;
#[cfg(feature = "safaridriver")]
mod safaridriver;

// TODO: DriverProcess
#[async_trait::async_trait]
pub trait Driver: Sized {
    async fn run(&self) -> Result<()>;

    fn addr(&self) -> Result<SocketAddr>;

    // TODO. How many clients max per process?
    fn limit(&self) -> Result<usize> {
        Ok(6)
    }

    async fn close(self) -> Result<()>;
}

pub enum Process {
    #[cfg(feature = "chromedriver")]
    Chrome(ChromeDriver),
    #[cfg(feature = "geckodriver")]
    Gecko(GeckoDriver),
    #[cfg(feature = "msedgedriver")]
    MsEdge(MsEdgeDriver),
    #[cfg(feature = "safaridriver")]
    Safari(SafariDriver),
}

#[cfg(feature = "chromedriver")]
impl From<ChromeDriver> for Process {
    fn from(value: ChromeDriver) -> Self {
        Process::Chrome(value)
    }
}

#[cfg(feature = "geckodriver")]
impl From<GeckoDriver> for Process {
    fn from(value: GeckoDriver) -> Self {
        Process::Gecko(value)
    }
}

#[cfg(feature = "msedgedriver")]
impl From<MsEdgeDriver> for Process {
    fn from(value: MsEdgeDriver) -> Self {
        Process::MsEdge(value)
    }
}

#[cfg(feature = "safaridriver")]
impl From<SafariDriver> for Process {
    fn from(value: SafariDriver) -> Self {
        Process::Safari(value)
    }
}

#[async_trait::async_trait]
impl Driver for Process {
    async fn run(&self) -> Result<()> {
        match self {
            #[cfg(feature = "chromedriver")]
            Process::Chrome(x) => x.run().await,
            #[cfg(feature = "geckodriver")]
            Process::Gecko(x) => x.run().await,
            #[cfg(feature = "msedgedriver")]
            Process::MsEdge(x) => x.run().await,
            #[cfg(feature = "safaridriver")]
            Process::Safari(x) => x.run().await,
        }
    }

    fn addr(&self) -> Result<SocketAddr> {
        match self {
            #[cfg(feature = "chromedriver")]
            Process::Chrome(x) => x.addr(),
            #[cfg(feature = "geckodriver")]
            Process::Gecko(x) => x.addr(),
            #[cfg(feature = "msedgedriver")]
            Process::MsEdge(x) => x.addr(),
            #[cfg(feature = "safaridriver")]
            Process::Safari(x) => x.addr(),
        }
    }

    fn limit(&self) -> Result<usize> {
        match self {
            #[cfg(feature = "chromedriver")]
            Process::Chrome(x) => x.limit(),
            #[cfg(feature = "geckodriver")]
            Process::Gecko(x) => x.limit(),
            #[cfg(feature = "msedgedriver")]
            Process::MsEdge(x) => x.limit(),
            #[cfg(feature = "safaridriver")]
            Process::Safari(x) => x.limit(),
        }
    }

    async fn close(self) -> Result<()> {
        match self {
            #[cfg(feature = "chromedriver")]
            Process::Chrome(x) => x.close().await,
            #[cfg(feature = "geckodriver")]
            Process::Gecko(x) => x.close().await,
            #[cfg(feature = "msedgedriver")]
            Process::MsEdge(x) => x.close().await,
            #[cfg(feature = "safaridriver")]
            Process::Safari(x) => x.close().await,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AnyBuilder {
    #[cfg(feature = "geckodriver")]
    Gecko(GeckoBuilder),
}

impl AnyBuilder {
    pub fn into_process(self) -> Process {
        match self {
            #[cfg(feature = "geckodriver")]
            AnyBuilder::Gecko(x) => Process::Gecko(x.build()),
        }
    }
}
