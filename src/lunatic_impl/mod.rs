pub use self::body::Body;
pub use self::client::{Client, ClientBuilder};
pub use self::request::{Request, RequestBuilder};
pub use self::response::HttpResponse;
// pub use self::upgrade::Upgraded;

#[cfg(feature = "blocking")]
pub(crate) use self::decoder::Decoder;

pub mod body;
pub mod client;
pub mod decoder;
mod http_stream;
#[cfg(feature = "multipart")]
pub mod multipart;
pub(crate) mod request;
mod response;
// mod upgrade;
