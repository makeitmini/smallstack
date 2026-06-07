mod error;
mod handler;
mod mime;
mod resolve;
mod server;

pub use error::StaticError;
pub use handler::{Handler, RequestInfo, ResponseBody};
pub use mime::mime_type;
pub use resolve::resolve;
pub use server::Server;
