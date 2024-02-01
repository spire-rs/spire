use std::ffi::{OsStr, OsString};
use std::ops::DerefMut;
use std::process::Stdio;
use std::sync::Arc;

use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::{Error, Result};

#[derive(Debug)]
enum HandlerInner {
    Created(Command),
    Spawned(Child),
}

#[derive(Debug, Clone)]
pub(crate) struct Handler(Arc<Mutex<HandlerInner>>);

impl Handler {
    pub fn new(exec: &OsStr, args: &[OsString]) -> Self {
        let mut command = Command::new(exec);
        command.args(args).kill_on_drop(true);

        #[cfg(feature = "stdio-null")]
        {
            command
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .stdin(Stdio::null());
        }

        let inner = HandlerInner::Created(command);
        let handler = Arc::new(Mutex::new(inner));
        Handler(handler)
    }

    pub async fn run(&self) -> Result<()> {
        let mut guard = self.0.lock().await;
        match guard.deref_mut() {
            HandlerInner::Created(x) => {
                let child = x.spawn()?;
                *guard = HandlerInner::Spawned(child);
                Ok(())
            }
            HandlerInner::Spawned(_) => Err(Error::FailedToStart),
        }
    }

    pub async fn close(self) -> Result<()> {
        let mut guard = self.0.lock().await;
        if let HandlerInner::Spawned(ref mut x) = guard.deref_mut() {
            x.kill().await?;
        }

        Ok(())
    }
}

// pub enum DriverKind {
//     #[cfg(feature = "chromedriver")]
//     ChromeDriver,
//     #[cfg(feature = "geckodriver")]
//     GeckoDriver,
//     #[cfg(feature = "msedgedriver")]
//     MsEdgeDriver,
//     #[cfg(feature = "safaridriver")]
//     SafariDriver,
// }
