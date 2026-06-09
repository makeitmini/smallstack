mod error;
mod result;

#[cfg(feature = "serde")]
mod ser;

#[cfg(feature = "serde")]
pub use ser::test_support;

pub use error::Error;
pub use result::{ErrorExt, Result};
