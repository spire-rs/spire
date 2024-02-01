pub(crate) use crate::handler::*;
pub use crate::process::*;

mod handler;
mod process;

// TODO: Download.
// TODO: Install.

/// Unrecoverable failure during [`DriverProcess`] execution.
///
/// This may be extended in the future so exhaustive matching is discouraged.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to find")]
    FailedToFind(#[from] std::io::Error),
    #[error("failed to start")]
    FailedToStart,
    #[error("failed to start")]
    FailedToAbort,
}

/// A specialized [`Result`] type for [`DriverProcess`] operations.
///
/// [`Result`]: std::result::Result
pub type Result<T> = std::result::Result<T, Error>;

pub struct Driver<T>
    where
        T: DriverProcess,
{
    driver: T,
}

impl<T> Driver<T>
    where
        T: DriverProcess,
{
    pub fn new(driver: T) -> Self {
        Self { driver }
    }

    pub async fn run(&self) -> Result<()> {
        self.driver.run().await
    }
}

impl<T> Default for Driver<T>
    where
        T: DriverProcess,
{
    fn default() -> Self {
        let driver = T::builder().build();
        Self::new(driver)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    #[cfg(feature = "chromedriver")]
    async fn run_geckodriver() -> Result<()> {
        let driver = Driver::<ChromeDriver>::default();
        driver.run().await
    }

    #[tokio::test]
    #[cfg(feature = "geckodriver")]
    async fn run_geckodriver() -> Result<()> {
        let driver = Driver::<GeckoDriver>::default();
        driver.run().await
    }

    #[tokio::test]
    #[cfg(feature = "msedgedriver")]
    async fn run_geckodriver() -> Result<()> {
        let driver = Driver::<EdgeDriver>::default();
        driver.run().await
    }

    #[tokio::test]
    #[cfg(feature = "safaridriver")]
    async fn run_geckodriver() -> Result<()> {
        let driver = Driver::<SafariDriver>::default();
        driver.run().await
    }
}
