#![deny(warnings)]

mod bounds;
mod document;
mod error;
mod fields;
mod index;
mod numkey;
mod score;
mod tokenizer;

pub use bounds::{BM25_B, BM25_K1, MAX_QUERY_BYTES};
pub use document::Document;
pub use error::{Error, Result};
pub use fields::{FieldConfig, FieldType, Visibility};
pub use index::{Comparison, ExactIndex, InvertedIndex, NumericIndex};
pub use numkey::NumKey;
pub use score::score_text;
pub use tokenizer::Tokenizer;
