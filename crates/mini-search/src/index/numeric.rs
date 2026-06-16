use crate::numkey::NumKey;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Comparison {
    Eq,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Debug, Clone)]
pub struct NumericIndex {
    fields: HashMap<String, BTreeMap<NumKey, HashSet<String>>>,
}

impl NumericIndex {
    pub fn new() -> Self {
        NumericIndex {
            fields: HashMap::new(),
        }
    }

    pub fn insert(&mut self, field: &str, key: NumKey, doc_id: &str) {
        self.fields
            .entry(field.to_string())
            .or_default()
            .entry(key)
            .or_default()
            .insert(doc_id.to_string());
    }

    pub fn remove(&mut self, field: &str, doc_id: &str) {
        if let Some(tree) = self.fields.get_mut(field) {
            tree.retain(|_, docs| {
                docs.remove(doc_id);
                !docs.is_empty()
            });
            if tree.is_empty() {
                self.fields.remove(field);
            }
        }
    }

    pub fn compare(&self, field: &str, op: Comparison, value: f64) -> HashSet<String> {
        let key = match NumKey::new(value) {
            Ok(k) => k,
            Err(_) => return HashSet::new(),
        };
        let tree = match self.fields.get(field) {
            Some(t) => t,
            None => return HashSet::new(),
        };
        match op {
            Comparison::Eq => tree.get(&key).cloned().unwrap_or_default(),
            Comparison::Gt => {
                let mut result = HashSet::new();
                for (_, docs) in tree.range((std::ops::Bound::Excluded(key), std::ops::Bound::Unbounded)) {
                    result.extend(docs.iter().cloned());
                }
                result
            }
            Comparison::Gte => {
                let mut result = HashSet::new();
                for (_, docs) in tree.range((std::ops::Bound::Included(key), std::ops::Bound::Unbounded)) {
                    result.extend(docs.iter().cloned());
                }
                result
            }
            Comparison::Lt => {
                let mut result = HashSet::new();
                for (_, docs) in tree.range((std::ops::Bound::Unbounded, std::ops::Bound::Excluded(key))) {
                    result.extend(docs.iter().cloned());
                }
                result
            }
            Comparison::Lte => {
                let mut result = HashSet::new();
                for (_, docs) in tree.range((std::ops::Bound::Unbounded, std::ops::Bound::Included(key))) {
                    result.extend(docs.iter().cloned());
                }
                result
            }
        }
    }

    pub fn range(&self, field: &str, low: f64, high: f64) -> HashSet<String> {
        let low_key = match NumKey::new(low) {
            Ok(k) => k,
            Err(_) => return HashSet::new(),
        };
        let high_key = match NumKey::new(high) {
            Ok(k) => k,
            Err(_) => return HashSet::new(),
        };
        let tree = match self.fields.get(field) {
            Some(t) => t,
            None => return HashSet::new(),
        };
        let mut result = HashSet::new();
        for (_, docs) in tree.range((
            std::ops::Bound::Included(low_key),
            std::ops::Bound::Included(high_key),
        )) {
            result.extend(docs.iter().cloned());
        }
        result
    }
}

impl Default for NumericIndex {
    fn default() -> Self {
        Self::new()
    }
}
