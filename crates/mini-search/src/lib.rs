#![deny(warnings)]

mod document;
mod error;
mod fields;

pub use document::Document;
pub use error::{Error, Result};
pub use fields::{FieldConfig, FieldType, Visibility};
