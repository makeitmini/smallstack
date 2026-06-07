mod error;
mod mime;
mod resolve;
mod server;

pub use error::StaticError;
pub use mime::mime_type;
pub use resolve::resolve;
pub use server::Server;
