use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FieldType {
    Text,
    TextArray,
    Keyword,
    Boolean,
    Tags,
    Integer,
    Float,
    Date,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Visibility {
    Indexed,
    Stored,
    Hidden,
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Indexed
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldConfig {
    pub field_type: FieldType,
    #[serde(default = "default_boost")]
    pub boost: f32,
    #[serde(default = "default_true")]
    pub searchable: bool,
    #[serde(default)]
    pub visibility: Visibility,
}

fn default_boost() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

impl FieldConfig {
    pub fn new(field_type: FieldType) -> Self {
        Self {
            field_type,
            boost: 1.0,
            searchable: true,
            visibility: Visibility::Indexed,
        }
    }
}
