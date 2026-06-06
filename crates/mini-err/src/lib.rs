mod error;
mod result;

#[cfg(feature = "serde")]
mod ser;

pub use error::Error;
pub use result::{ErrorExt, Result};
