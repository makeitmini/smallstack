use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct ExactIndex {
    fields: HashMap<String, HashMap<String, HashSet<String>>>,
}

impl ExactIndex {
    pub fn new() -> Self {
        ExactIndex {
            fields: HashMap::new(),
        }
    }

    pub fn insert(&mut self, field: &str, value: &str, doc_id: &str) {
        self.fields
            .entry(field.to_string())
            .or_default()
            .entry(value.to_string())
            .or_default()
            .insert(doc_id.to_string());
    }

    pub fn remove(&mut self, field: &str, doc_id: &str) {
        if let Some(values) = self.fields.get_mut(field) {
            values.retain(|_, docs| {
                docs.remove(doc_id);
                !docs.is_empty()
            });
            if values.is_empty() {
                self.fields.remove(field);
            }
        }
    }

    pub fn matching(&self, field: &str, value: &str) -> HashSet<String> {
        self.fields
            .get(field)
            .and_then(|values| values.get(value))
            .cloned()
            .unwrap_or_default()
    }
}

impl Default for ExactIndex {
    fn default() -> Self {
        Self::new()
    }
}
