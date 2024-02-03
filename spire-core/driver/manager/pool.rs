use std::convert::Infallible;
use std::net::SocketAddr;

use deadpool::managed::{Manager, Metrics, Object, Pool, RecycleResult};
use fantoccini::Client;

use crate::driver::manager::Browser;
use crate::driver::process::{ChromeDriver, Driver, GeckoDriver, MsEdgeDriver, Process, SafariDriver};

pub struct DriverManager2<T>
where
    T: Driver + Sync,
{
    managed: Vec<T>,
}

impl<T> DriverManager2<T>
where
    T: Driver + Sync,
{
    pub fn new() -> Self {
        Self::default()
    }
}

impl<T> Default for DriverManager2<T>
where
    T: Driver + Sync,
{
    fn default() -> Self {
        todo!()
    }
}

#[deadpool::async_trait]
impl<T> Manager for DriverManager2<T>
where
    T: Driver + Sync + Send,
{
    type Type = Client;
    type Error = Infallible;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        todo!()
    }

    async fn recycle(
        &self,
        _client: &mut Self::Type,
        _metrics: &Metrics,
    ) -> RecycleResult<Self::Error> {
        // TODO. Check if closed and reconnect if needed.
        Ok(())
    }
}

pub struct DriverManager {}

#[deadpool::async_trait]
impl Manager for DriverManager {
    type Type = Client;
    type Error = Infallible;

    async fn create(&self) -> Result<Self::Type, Self::Error> {
        todo!()
    }

    async fn recycle(&self, obj: &mut Self::Type, metrics: &Metrics) -> RecycleResult<Self::Error> {
        todo!()
    }
}

pub struct DriverPool {
    manager: Pool<DriverManager>,
    chromedriver: Pool<DriverManager2<ChromeDriver>>,
    geckodriver: Pool<DriverManager2<GeckoDriver>>,
    msedgedriver: Pool<DriverManager2<MsEdgeDriver>>,
    safaridriver: Pool<DriverManager2<SafariDriver>>,
}

impl DriverPool {
    pub fn new() -> Self {
        todo!()
    }

    pub fn add<T>(self, process: T) -> Self
    where
        T: Into<Process>,
        // T: Driver,
    {
        todo!()
    }

    pub fn connect<T>(self, browser: Browser, address: T) -> Self
    where
        T: Into<SocketAddr>,
    {
        let address  = address.into();
        todo!()
    }

    pub async fn get(&self, browser: Browser) -> Object<DriverManager2<ChromeDriver>> {
        let manager = self.manager.get().await.unwrap();


        match browser {
            Browser::Chrome => {}
            Browser::Edge => {}
            Browser::Firebox => {}
            Browser::Safari => {}
        }

        // self.safaridriver.g
        // self.pool.

        todo!()
    }
}

impl Default for DriverPool {
    fn default() -> Self {
        todo!()
    }
}
