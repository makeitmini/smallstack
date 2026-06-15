use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NumKey(f64);

impl NumKey {
    pub fn new(value: f64) -> Result<Self> {
        if value.is_nan() {
            return Err(Error::invalid_value("NaN is not a valid NumKey"));
        }
        Ok(NumKey(value))
    }
}

impl From<i64> for NumKey {
    fn from(value: i64) -> Self {
        NumKey(value as f64)
    }
}

impl PartialEq for NumKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for NumKey {}

impl PartialOrd for NumKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NumKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}
