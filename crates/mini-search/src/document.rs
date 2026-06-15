use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Document {
    pub id: String,
    #[serde(flatten)]
    pub fields: HashMap<String, Value>,
}

impl Document {
    pub fn new(id: impl Into<String>, fields: HashMap<String, Value>) -> Self {
        Self {
            id: id.into(),
            fields,
        }
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.fields.get(name)
    }
}
