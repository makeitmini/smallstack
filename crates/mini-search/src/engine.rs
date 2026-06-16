use crate::bounds::MAX_RESULTS;
use crate::document::Document;
use crate::error::{Error, Result};
use crate::fields::{FieldConfig, FieldType, Visibility};
use crate::index::{ExactIndex, InvertedIndex, NumericIndex};
use crate::numkey::NumKey;
use crate::processor::Processor;
use crate::query::{Filter, Query};
use crate::score::score_text;
use crate::tokenizer::Tokenizer;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
#[cfg(feature = "persist")]
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct FieldContribution {
    pub field_name: String,
    pub term: String,
    pub score_component: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub doc: Document,
    pub score: f32,
    #[serde(default)]
    pub field_contributions: Vec<FieldContribution>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExplainHit {
    pub doc: Document,
    pub score: f32,
    pub source_collection: String,
    pub field_contributions: Vec<FieldContribution>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchMetrics {
    pub total_results: usize,
}

pub struct Engine {
    pub(crate) documents: HashMap<String, HashMap<String, Document>>,
    pub(crate) field_configs: HashMap<String, HashMap<String, FieldConfig>>,
    pub(crate) inverted: HashMap<String, InvertedIndex>,
    pub(crate) numeric: HashMap<String, NumericIndex>,
    pub(crate) exact: HashMap<String, ExactIndex>,
    pub(crate) tokenizer: Tokenizer,
    #[cfg(feature = "persist")]
    pub(crate) storage_dir: Option<PathBuf>,
    pipeline: Vec<Box<dyn Processor>>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(not(feature = "persist"))]
        {
            f.debug_struct("Engine")
                .field("documents", &self.documents)
                .field("field_configs", &self.field_configs)
                .field("inverted", &self.inverted)
                .field("numeric", &self.numeric)
                .field("exact", &self.exact)
                .field("tokenizer", &self.tokenizer)
                .field("pipeline_len", &self.pipeline.len())
                .finish()
        }
        #[cfg(feature = "persist")]
        {
            f.debug_struct("Engine")
                .field("documents", &self.documents)
                .field("field_configs", &self.field_configs)
                .field("inverted", &self.inverted)
                .field("numeric", &self.numeric)
                .field("exact", &self.exact)
                .field("tokenizer", &self.tokenizer)
                .field("pipeline_len", &self.pipeline.len())
                .field("storage_dir", &self.storage_dir)
                .finish()
        }
    }
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            documents: HashMap::new(),
            field_configs: HashMap::new(),
            inverted: HashMap::new(),
            numeric: HashMap::new(),
            exact: HashMap::new(),
            tokenizer: Tokenizer::new(),
            #[cfg(feature = "persist")]
            storage_dir: None,
            pipeline: Vec::new(),
        }
    }

    pub fn configure_fields(&mut self, collection: &str, cfgs: HashMap<String, FieldConfig>) {
        self.field_configs.insert(collection.to_string(), cfgs);
    }

    pub fn add_document(&mut self, collection: &str, doc: Document) -> Result<()> {
        let cfgs = self
            .field_configs
            .get(collection)
            .ok_or_else(|| Error::invalid_query(format!("unconfigured collection '{}'", collection)))?;

        let inv = self.inverted.entry(collection.to_string()).or_default();
        inv.insert(&doc, cfgs, &self.tokenizer)?;

        for (field_name, cfg) in cfgs {
            let value = match doc.get(field_name) {
                Some(v) => v,
                None => continue,
            };
            match cfg.field_type {
                FieldType::Integer | FieldType::Float | FieldType::Date => {
                    if let Some(num) = value.as_f64() {
                        if let Ok(key) = NumKey::new(num) {
                            self.numeric
                                .entry(collection.to_string())
                                .or_default()
                                .insert(field_name, key, &doc.id);
                        }
                    }
                }
                FieldType::Keyword | FieldType::Tags => {
                    match value {
                        Value::String(s) => {
                            self.exact
                                .entry(collection.to_string())
                                .or_default()
                                .insert(field_name, s, &doc.id);
                        }
                        Value::Array(arr) => {
                            let exact = self.exact.entry(collection.to_string()).or_default();
                            for v in arr {
                                if let Some(s) = v.as_str() {
                                    exact.insert(field_name, s, &doc.id);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                FieldType::Boolean => {
                    let s = match value {
                        Value::Bool(b) => Some(if *b { "true" } else { "false" }),
                        Value::String(s) => Some(s.as_str()),
                        _ => None,
                    };
                    if let Some(s) = s {
                        self.exact
                            .entry(collection.to_string())
                            .or_default()
                            .insert(field_name, s, &doc.id);
                    }
                }
                _ => {}
            }
        }

        self.documents
            .entry(collection.to_string())
            .or_default()
            .insert(doc.id.clone(), doc);

        Ok(())
    }

    pub fn add_processor(&mut self, p: impl Processor + 'static) {
        self.pipeline.push(Box::new(p));
    }

    pub fn lookup(&self, collection: &str, doc_id: &str) -> Option<Document> {
        let cfgs = self.field_configs.get(collection)?;
        let doc = self.documents.get(collection)?.get(doc_id)?;
        Some(redact_document(doc, cfgs))
    }

    pub fn search(
        &self,
        collection: &str,
        query_str: &str,
    ) -> Result<(Vec<SearchHit>, SearchMetrics)> {
        let cfgs = self
            .field_configs
            .get(collection)
            .ok_or_else(|| Error::not_found("collection", collection))?;

        let query_str = self.run_pre_search(query_str)?;
        let query = Query::parse(&query_str)?;

        for filter in &query.filters {
            validate_filter(filter, cfgs)?;
        }

        let text_candidates = if query.text.is_empty() {
            None
        } else {
            let mut candidates = HashSet::new();
            if let Some(inv) = self.inverted.get(collection) {
                for clause in &query.text {
                    let fields = text_search_fields(cfgs, clause.field.as_deref());
                    for field in &fields {
                        for (doc_id, _) in inv.postings(field, &clause.term) {
                            candidates.insert(doc_id);
                        }
                    }
                }
            }
            Some(candidates)
        };

        let filter_candidates = if query.filters.is_empty() {
            None
        } else {
            let mut sets: Vec<HashSet<String>> = Vec::new();
            for filter in &query.filters {
                let set = match filter {
                    Filter::Compare { field, op, value } => self
                        .numeric
                        .get(collection)
                        .map(|n| n.compare(field, *op, *value))
                        .unwrap_or_default(),
                    Filter::Range { field, low, high } => self
                        .numeric
                        .get(collection)
                        .map(|n| n.range(field, *low, *high))
                        .unwrap_or_default(),
                    Filter::Exact { field, value } => self
                        .exact
                        .get(collection)
                        .map(|e| e.matching(field, value))
                        .unwrap_or_default(),
                };
                sets.push(set);
            }
            let mut iter = sets.into_iter();
            let mut result = iter.next().unwrap_or_default();
            for set in iter {
                result.retain(|id| set.contains(id));
            }
            Some(result)
        };

        let candidates = match (filter_candidates, text_candidates) {
            (Some(f), Some(t)) => {
                f.intersection(&t).cloned().collect::<HashSet<_>>()
            }
            (Some(f), None) => f,
            (None, Some(t)) => t,
            (None, None) => self
                .documents
                .get(collection)
                .map(|docs| docs.keys().cloned().collect())
                .unwrap_or_default(),
        };

        let (scores, contributions) = if !query.text.is_empty() {
            if let Some(inv) = self.inverted.get(collection) {
                score_collection(&query, inv, cfgs, &candidates)
            } else {
                (HashMap::new(), HashMap::new())
            }
        } else {
            (HashMap::new(), HashMap::new())
        };
        let mut scored: Vec<(String, f32)> = if query.text.is_empty() {
            candidates.into_iter().map(|id| (id, 0.0)).collect()
        } else {
            scores.into_iter().collect()
        };
        self.apply_value_boosts(&mut scored, cfgs, collection);
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored.truncate(MAX_RESULTS);

        let docs = self
            .documents
            .get(collection)
            .ok_or_else(|| Error::not_found("collection", collection))?;

        let hits: Vec<SearchHit> = scored
            .into_iter()
            .filter_map(|(id, score)| {
                docs.get(&id).map(|doc| SearchHit {
                    doc: redact_document(doc, cfgs),
                    score,
                    field_contributions: contributions
                        .get(&id)
                        .cloned()
                        .unwrap_or_default(),
                })
            })
            .collect();

        let hits = self.run_post_search(hits)?;

        let metrics = SearchMetrics {
            total_results: hits.len(),
        };

        Ok((hits, metrics))
    }

    pub fn search_multi(
        &self,
        collections: &[&str],
        query_str: &str,
    ) -> Result<(Vec<SearchHit>, SearchMetrics)> {
        let mut merged: HashMap<String, (f32, Document)> = HashMap::new();

        for collection in collections {
            let (hits, _) = self.search(collection, query_str)?;
            for hit in hits {
                merged
                    .entry(hit.doc.id.clone())
                    .and_modify(|existing| {
                        if hit.score > existing.0 {
                            existing.0 = hit.score;
                            existing.1 = hit.doc.clone();
                        }
                    })
                    .or_insert((hit.score, hit.doc));
            }
        }

        let mut hits: Vec<SearchHit> = merged
            .into_iter()
            .map(|(_, (score, doc))| SearchHit {
                doc,
                score,
                field_contributions: Vec::new(),
            })
            .collect();
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(MAX_RESULTS);

        let metrics = SearchMetrics {
            total_results: hits.len(),
        };
        Ok((hits, metrics))
    }

    pub fn explain(
        &self,
        collection: &str,
        query_str: &str,
        doc_id: &str,
    ) -> Option<ExplainHit> {
        let cfgs = self.field_configs.get(collection)?;
        let doc = self.documents.get(collection)?.get(doc_id)?;
        let query = Query::parse(query_str).ok()?;

        let mut candidates = HashSet::new();
        candidates.insert(doc_id.to_string());

        let (scores, mut contributions) = if !query.text.is_empty() {
            if let Some(inv) = self.inverted.get(collection) {
                score_collection(&query, inv, cfgs, &candidates)
            } else {
                (HashMap::new(), HashMap::new())
            }
        } else {
            (HashMap::new(), HashMap::new())
        };

        let raw_score = scores.get(doc_id).copied().unwrap_or(0.0);
        let mut scored = vec![(doc_id.to_string(), raw_score)];
        self.apply_value_boosts(&mut scored, cfgs, collection);

        Some(ExplainHit {
            doc: redact_document(doc, cfgs),
            score: scored[0].1,
            source_collection: collection.to_string(),
            field_contributions: contributions.remove(doc_id).unwrap_or_default(),
        })
    }
}

impl Engine {
    fn run_pre_search(&self, query_str: &str) -> Result<String> {
        let mut query = query_str.to_string();
        for processor in &self.pipeline {
            query = processor.pre_search(&query)?;
        }
        Ok(query)
    }

    fn run_post_search(&self, hits: Vec<SearchHit>) -> Result<Vec<SearchHit>> {
        let mut hits = hits;
        for processor in &self.pipeline {
            hits = processor.post_search(hits)?;
        }
        Ok(hits)
    }

    fn apply_value_boosts(
        &self,
        scored: &mut [(String, f32)],
        cfgs: &HashMap<String, FieldConfig>,
        collection: &str,
    ) {
        let docs = match self.documents.get(collection) {
            Some(d) => d,
            None => return,
        };
        for (doc_id, score) in scored.iter_mut() {
            let doc = match docs.get(doc_id) {
                Some(d) => d,
                None => continue,
            };
            for (field_name, cfg) in cfgs {
                if cfg.value_boosts.is_empty() {
                    continue;
                }
                let value_str = match doc.get(field_name) {
                    Some(serde_json::Value::String(s)) => s.clone(),
                    Some(serde_json::Value::Bool(b)) => {
                        (if *b { "true" } else { "false" }).to_string()
                    }
                    Some(serde_json::Value::Number(n)) => n.to_string(),
                    _ => continue,
                };
                if let Some(multiplier) = cfg.value_boosts.get(&value_str) {
                    *score *= multiplier;
                }
            }
        }
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

fn redact_document(doc: &Document, cfgs: &HashMap<String, FieldConfig>) -> Document {
    let hidden: HashSet<&str> = cfgs
        .iter()
        .filter(|(_, cfg)| cfg.visibility == Visibility::Hidden)
        .map(|(name, _)| name.as_str())
        .collect();

    if hidden.is_empty() {
        return doc.clone();
    }

    let fields: HashMap<String, Value> = doc
        .fields
        .iter()
        .filter(|(name, _)| !hidden.contains(name.as_str()))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();

    Document::new(doc.id.clone(), fields)
}

fn validate_filter(filter: &Filter, cfgs: &HashMap<String, FieldConfig>) -> Result<()> {
    let field = match filter {
        Filter::Compare { field, .. } => field,
        Filter::Range { field, .. } => field,
        Filter::Exact { field, .. } => field,
    };
    let cfg = cfgs
        .get(field)
        .ok_or_else(|| Error::invalid_query(format!("unknown field '{}'", field)))?;

    match filter {
        Filter::Compare { .. } | Filter::Range { .. } => match cfg.field_type {
            FieldType::Integer | FieldType::Float | FieldType::Date => Ok(()),
            _ => Err(Error::invalid_query(format!(
                "field '{}' does not support comparisons",
                field
            ))),
        },
        Filter::Exact { .. } => Ok(()),
    }
}

fn score_collection(
    query: &Query,
    inv: &InvertedIndex,
    cfgs: &HashMap<String, FieldConfig>,
    candidates: &HashSet<String>,
) -> (HashMap<String, f32>, HashMap<String, Vec<FieldContribution>>) {
    let mut scores: HashMap<String, f32> = HashMap::new();
    let mut contributions: HashMap<String, Vec<FieldContribution>> = HashMap::new();

    for clause in &query.text {
        let fields = text_search_fields(cfgs, clause.field.as_deref());
        for field in &fields {
            let boost = cfgs.get(field).map(|c| c.boost).unwrap_or(1.0);
            let field_scores = score_text(inv, field, &[clause.term.clone()], boost);
            for (doc_id, score) in field_scores {
                if candidates.contains(&doc_id) {
                    *scores.entry(doc_id.clone()).or_insert(0.0) += score;
                    contributions
                        .entry(doc_id.clone())
                        .or_default()
                        .push(FieldContribution {
                            field_name: field.clone(),
                            term: clause.term.clone(),
                            score_component: score,
                        });
                }
            }
        }
    }

    (scores, contributions)
}

fn text_search_fields(
    cfgs: &HashMap<String, FieldConfig>,
    field: Option<&str>,
) -> Vec<String> {
    match field {
        Some(f) => {
            if cfgs.get(f).map_or(false, |c| {
                c.searchable && matches!(c.field_type, FieldType::Text | FieldType::TextArray)
            }) {
                vec![f.to_string()]
            } else {
                vec![]
            }
        }
        None => cfgs
            .iter()
            .filter(|(_, c)| {
                c.searchable && matches!(c.field_type, FieldType::Text | FieldType::TextArray)
            })
            .map(|(name, _)| name.clone())
            .collect(),
    }
}
