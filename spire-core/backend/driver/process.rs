use std::fmt;
use std::sync::Mutex;

use crate::BoxError;

enum ConnType {
    Managed(BrowserProcess),
    Unmanaged(String),
}

#[async_trait::async_trait]
pub trait BrowserDriver {
    type Error: Into<BoxError>;

    fn connect(&mut self) -> Result<(), Self::Error>;
    fn address(&self) -> Result<String, Self::Error>;
}

pub struct BoxBrowserDriver {
    inner: Box<dyn BrowserDriver<Error = BoxError> + Send>,
}

impl BoxBrowserDriver {
    pub fn new<T, E>(driver: T) -> Self
    where
        T: BrowserDriver<Error = E>,
        E: Into<BoxError>,
    {
        todo!()
    }
}

impl BrowserDriver for BoxBrowserDriver {
    type Error = BoxError;

    fn connect(&mut self) -> Result<(), Self::Error> {
        self.inner.connect().map_err(BoxError::from)
    }

    fn address(&self) -> Result<String, Self::Error> {
        self.inner.address().map_err(BoxError::from)
    }
}

#[must_use]
pub struct BrowserProcess {
    driver: Mutex<BoxBrowserDriver>,
}

impl BrowserProcess {
    /// Creates a new [`BrowserProcess`].
    pub fn new<T>(driver: T) -> Self
    where
        T: BrowserDriver + Send + 'static,
    {
        // let boxed: BoxBrowserDriver = Box::new(driver);
        // let driver = Mutex::new(boxed);
        // Self { driver }

        todo!()
    }

    /// Returns the underlying boxed [`BrowserDriver`].
    pub fn into_inner(self) -> BoxBrowserDriver {
        self.driver.into_inner().unwrap()
    }
}

impl fmt::Debug for BrowserProcess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserProcess").finish_non_exhaustive()
    }
}
