use std::net::SocketAddr;

#[cfg(feature = "chromedriver")]
pub use chromedriver::ChromeDriver;
#[cfg(feature = "geckodriver")]
pub use geckodriver::GeckoDriver;
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

// TODO: ProcessBuilder
pub trait Build<T: DriverProcess>: Default {
    fn build(self) -> T;
}

// TODO: DriverProcess
#[async_trait::async_trait]
pub trait DriverProcess: Sized {
    type Builder: Build<Self>;

    fn builder() -> Self::Builder {
        <Self::Builder as Default>::default()
    }

    async fn run(&self) -> Result<()>;

    async fn addr(&self) -> Result<SocketAddr>;

    async fn close(self) -> Result<()>;
}
