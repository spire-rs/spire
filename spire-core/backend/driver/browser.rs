/// Extension trait for Backend::Client
/// ... for backends that run actual browsers
pub trait BrowserBackend {}

pub trait WebDriver {}

// pub struct ChromeDriver {}

pub enum Connection {
    Managed(DriverProcess),
    Unmanaged(String),
}

pub struct DriverProcess {}

pub struct ClientHandler {
    id: u64,
    client: (),
    conn: (),
}
