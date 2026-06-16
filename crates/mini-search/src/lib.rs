#![deny(warnings)]

mod bounds;
mod document;
mod error;
mod fields;
mod numkey;
mod tokenizer;

pub use bounds::MAX_QUERY_BYTES;
pub use document::Document;
pub use error::{Error, Result};
pub use fields::{FieldConfig, FieldType, Visibility};
pub use numkey::NumKey;
pub use tokenizer::Tokenizer;
