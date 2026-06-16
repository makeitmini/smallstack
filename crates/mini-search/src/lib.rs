#![deny(warnings)]

mod bounds;
mod document;
mod engine;
mod error;
mod fields;
mod index;
mod numkey;
#[cfg(feature = "persist")]
mod persist;
#[cfg(feature = "wasm")]
mod wasm;
mod query;
mod score;
mod tokenizer;

pub use bounds::{BM25_B, BM25_K1, MAX_CANDIDATES, MAX_QUERY_BYTES, MAX_QUERY_TERMS, MAX_RESULTS};
pub use document::Document;
pub use engine::{Engine, SearchHit, SearchMetrics};
pub use error::{Error, Result};
pub use fields::{FieldConfig, FieldType, Visibility};
pub use index::{Comparison, ExactIndex, InvertedIndex, NumericIndex};
pub use numkey::NumKey;
pub use query::{Filter, Query, TextClause};
pub use score::score_text;
pub use tokenizer::Tokenizer;
