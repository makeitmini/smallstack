#![deny(warnings)]

mod document;
mod error;
mod fields;
mod numkey;

pub use document::Document;
pub use error::{Error, Result};
pub use fields::{FieldConfig, FieldType, Visibility};
pub use numkey::NumKey;
