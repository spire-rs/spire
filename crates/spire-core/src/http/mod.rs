//! HTTP requests and responses, their crawl metadata, and the underlying
//! [`http`] crate.

mod meta;
mod request_ext;
mod response_ext;
mod uri_ext;

pub use ::http;

pub(crate) use self::meta::RequestMeta;
pub use self::request_ext::{Request, RequestExt};
pub use self::response_ext::{Response, ResponseExt};
pub use self::uri_ext::UriExt;
