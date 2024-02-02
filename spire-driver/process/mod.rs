use std::net::SocketAddr;

#[cfg(feature = "chromedriver")]
pub use chromedriver::ChromeDriver;
#[cfg(feature = "geckodriver")]
pub use geckodriver::{GeckoBuilder, GeckoDriver};
#[cfg(feature = "msedgedriver")]
pub use msedgedriver::EdgeDriver;
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
pub trait Process: Sized {
    async fn run(&self) -> Result<()>;

    async fn addr(&self) -> Result<SocketAddr>;

    async fn close(self) -> Result<()>;
}

pub enum AnyProcess {
    #[cfg(feature = "chromedriver")]
    Chrome(ChromeDriver),
    #[cfg(feature = "geckodriver")]
    Gecko(GeckoDriver),
    #[cfg(feature = "msedgedriver")]
    MsEdge(EdgeDriver),
    #[cfg(feature = "safaridriver")]
    Safari(SafariDriver),
}

#[async_trait::async_trait]
impl Process for AnyProcess {
    async fn run(&self) -> Result<()> {
        match self {
            #[cfg(feature = "chromedriver")]
            AnyProcess::Chrome(x) => x.run().await,
            #[cfg(feature = "geckodriver")]
            AnyProcess::Gecko(x) => x.run().await,
            #[cfg(feature = "msedgedriver")]
            AnyProcess::MsEdge(x) => x.run().await,
            #[cfg(feature = "safaridriver")]
            AnyProcess::Safari(x) => x.run().await,
        }
    }

    async fn addr(&self) -> Result<SocketAddr> {
        match self {
            #[cfg(feature = "chromedriver")]
            AnyProcess::Chrome(x) => x.addr().await,
            #[cfg(feature = "geckodriver")]
            AnyProcess::Gecko(x) => x.addr().await,
            #[cfg(feature = "msedgedriver")]
            AnyProcess::MsEdge(x) => x.addr().await,
            #[cfg(feature = "safaridriver")]
            AnyProcess::Safari(x) => x.addr().await,
        }
    }

    async fn close(self) -> Result<()> {
        match self {
            #[cfg(feature = "chromedriver")]
            AnyProcess::Chrome(x) => x.close().await,
            #[cfg(feature = "geckodriver")]
            AnyProcess::Gecko(x) => x.close().await,
            #[cfg(feature = "msedgedriver")]
            AnyProcess::MsEdge(x) => x.close().await,
            #[cfg(feature = "safaridriver")]
            AnyProcess::Safari(x) => x.close().await,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AnyBuilder {
    #[cfg(feature = "geckodriver")]
    Gecko(GeckoBuilder),
}

impl AnyBuilder {
    pub fn into_process(self) -> AnyProcess {
        match self {
            #[cfg(feature = "geckodriver")]
            AnyBuilder::Gecko(x) => AnyProcess::Gecko(x.build()),
        }
    }
}
