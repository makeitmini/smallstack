use crate::document::Document;
use crate::error::Result;
use crate::fields::{FieldConfig, FieldType};
use crate::tokenizer::Tokenizer;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct InvertedIndex {
    postings: HashMap<String, HashMap<String, Vec<(String, usize)>>>,
    field_lengths: HashMap<String, HashMap<String, usize>>,
}

impl InvertedIndex {
    pub fn new() -> Self {
        InvertedIndex {
            postings: HashMap::new(),
            field_lengths: HashMap::new(),
        }
    }

    pub fn insert(
        &mut self,
        doc: &Document,
        cfgs: &HashMap<String, FieldConfig>,
        tok: &Tokenizer,
    ) -> Result<()> {
        for (field_name, cfg) in cfgs {
            if !cfg.searchable {
                continue;
            }
            match cfg.field_type {
                FieldType::Text | FieldType::TextArray => {
                    let value = match doc.get(field_name) {
                        Some(v) => v,
                        None => continue,
                    };
                    let text = match value {
                        Value::String(s) => s.clone(),
                        Value::Array(arr) => arr
                            .iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(" "),
                        _ => continue,
                    };
                    let tokens = tok.tokenize(&text)?;
                    if tokens.is_empty() {
                        continue;
                    }
                    self.insert_tokens(&doc.id, field_name, &tokens);
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, doc_id: &str, field: &str) {
        if let Some(term_map) = self.postings.get_mut(field) {
            term_map.retain(|_, postings| {
                postings.retain(|(id, _)| id != doc_id);
                !postings.is_empty()
            });
            if term_map.is_empty() {
                self.postings.remove(field);
            }
        }
        if let Some(len_map) = self.field_lengths.get_mut(field) {
            len_map.remove(doc_id);
            if len_map.is_empty() {
                self.field_lengths.remove(field);
            }
        }
    }

    pub fn postings(&self, field: &str, term: &str) -> Vec<(String, usize)> {
        self.postings
            .get(field)
            .and_then(|terms| terms.get(term))
            .cloned()
            .unwrap_or_default()
    }

    pub fn field_len(&self, field: &str, doc_id: &str) -> usize {
        self.field_lengths
            .get(field)
            .and_then(|lens| lens.get(doc_id))
            .copied()
            .unwrap_or(0)
    }

    pub fn doc_freq(&self, field: &str, term: &str) -> usize {
        self.postings.get(field).and_then(|terms| terms.get(term)).map_or(0, |p| p.len())
    }

    pub fn num_docs(&self, field: &str) -> usize {
        self.field_lengths.get(field).map_or(0, |lens| lens.len())
    }

    pub fn total_field_len(&self, field: &str) -> usize {
        self.field_lengths
            .get(field)
            .map_or(0, |lens| lens.values().sum())
    }

    pub fn avg_field_len(&self, field: &str) -> f64 {
        let n = self.num_docs(field);
        if n == 0 {
            0.0
        } else {
            self.total_field_len(field) as f64 / n as f64
        }
    }

    fn insert_tokens(&mut self, doc_id: &str, field: &str, tokens: &[String]) {
        self.remove(doc_id, field);

        let mut tf_counts: HashMap<&str, usize> = HashMap::new();
        for token in tokens {
            *tf_counts.entry(token).or_insert(0) += 1;
        }

        let term_map = self.postings.entry(field.to_string()).or_default();
        for (term, tf) in &tf_counts {
            term_map
                .entry((*term).to_string())
                .or_default()
                .push((doc_id.to_string(), *tf));
        }

        self.field_lengths
            .entry(field.to_string())
            .or_default()
            .insert(doc_id.to_string(), tokens.len());
    }
}

impl Default for InvertedIndex {
    fn default() -> Self {
        Self::new()
    }
}
