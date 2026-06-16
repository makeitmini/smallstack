use crate::engine::{Engine, SearchHit, SearchMetrics};
use crate::fields::FieldConfig;
use crate::Document;
use serde::Serialize;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct SearchOutput {
    hits: Vec<SearchHit>,
    metrics: SearchMetrics,
}

#[wasm_bindgen]
pub struct JsEngine {
    inner: Engine,
}

#[wasm_bindgen]
impl JsEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> JsEngine {
        console_error_panic_hook::set_once();
        JsEngine {
            inner: Engine::new(),
        }
    }

    pub fn configure_fields(&mut self, collection: &str, json_config: &str) -> Result<(), JsValue> {
        let cfgs: HashMap<String, FieldConfig> =
            serde_json::from_str(json_config).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.inner.configure_fields(collection, cfgs);
        Ok(())
    }

    pub fn add_document_json(&mut self, collection: &str, json_doc: &str) -> Result<(), JsValue> {
        let doc: Document =
            serde_json::from_str(json_doc).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.inner
            .add_document(collection, doc)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn search_json(&self, collection: &str, query: &str) -> Result<String, JsValue> {
        let (hits, metrics) = self
            .inner
            .search(collection, query)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let output = SearchOutput { hits, metrics };
        serde_json::to_string(&output).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn lookup_json(&self, collection: &str, doc_id: &str) -> Result<String, JsValue> {
        match self.inner.lookup(collection, doc_id) {
            Some(doc) => {
                serde_json::to_string(&doc).map_err(|e| JsValue::from_str(&e.to_string()))
            }
            None => Ok("null".to_string()),
        }
    }
}

impl Default for JsEngine {
    fn default() -> Self {
        Self::new()
    }
}
