use mini_search::{Document, Engine, FieldConfig, FieldType, SearchHit};
use serde_json::json;
use std::collections::HashMap;

fn cfgs() -> HashMap<String, FieldConfig> {
    HashMap::from([
        ("notes".to_string(), FieldConfig::new(FieldType::Text)),
        ("dose_mg".to_string(), FieldConfig::new(FieldType::Float)),
    ])
}

fn doc_a() -> Document {
    let mut fields = HashMap::new();
    fields.insert("notes".to_string(), json!("dog"));
    fields.insert("dose_mg".to_string(), json!(3.0));
    Document::new("d_a", fields)
}

fn doc_b() -> Document {
    let mut fields = HashMap::new();
    fields.insert("notes".to_string(), json!("cat"));
    fields.insert("dose_mg".to_string(), json!(8.0));
    Document::new("d_b", fields)
}

fn ids(hits: &[SearchHit]) -> Vec<&str> {
    hits.iter().map(|h| h.doc.id.as_str()).collect()
}

fn setup() -> (Engine, HashMap<String, FieldConfig>) {
    let cfgs = cfgs();
    let mut engine = Engine::new();
    engine.configure_fields("meds", cfgs.clone());
    engine.add_document("meds", doc_a()).unwrap();
    engine.add_document("meds", doc_b()).unwrap();
    (engine, cfgs)
}

// --- Defect #3 regression: filter-only query with ZERO text clauses ---

#[test]
fn filter_only_query_returns_correct_docs() {
    let (engine, _) = setup();
    let (hits, _) = engine.search("meds", "dose_mg:>5").unwrap();
    assert_eq!(ids(&hits), vec!["d_b"]);
}

#[test]
fn text_intersected_with_filter_requires_both() {
    let (engine, _) = setup();
    // "dog" matches d_a, but d_a fails dose filter -> empty
    let (hits, _) = engine.search("meds", "dog dose_mg:>5").unwrap();
    assert!(hits.is_empty());
}

#[test]
fn text_intersected_with_satisfiable_filter() {
    let (engine, _) = setup();
    // "cat" matches d_b, and d_b satisfies dose filter
    let (hits, _) = engine.search("meds", "cat dose_mg:>5").unwrap();
    assert_eq!(ids(&hits), vec!["d_b"]);
}

// --- Support: add_document + lookup ---

#[test]
fn add_document_then_lookup_returns_doc() {
    let (engine, _) = setup();
    let doc = engine.lookup("meds", "d_a");
    assert!(doc.is_some());
    assert_eq!(doc.unwrap().id, "d_a");
    assert_eq!(doc.unwrap().get("dose_mg"), Some(&json!(3.0)));
}

// --- Support: configure_fields + add_document works ---

#[test]
fn configure_then_add_then_search() {
    let mut engine = Engine::new();
    engine.configure_fields("items", cfgs());
    let mut fields = HashMap::new();
    fields.insert("notes".to_string(), json!("test"));
    fields.insert("dose_mg".to_string(), json!(1.0));
    engine.add_document("items", Document::new("i1", fields)).unwrap();

    let (hits, _) = engine.search("items", "test").unwrap();
    assert_eq!(ids(&hits), vec!["i1"]);
}

// --- Error-path: search on unconfigured collection ---

#[test]
fn search_unconfigured_collection_returns_not_found() {
    let engine = Engine::new();
    let result = engine.search("ghost", "dog");
    assert!(matches!(result, Err(mini_search::Error::NotFound { .. })));
}

// --- Error-path: add to unconfigured collection ---

#[test]
fn add_document_unconfigured_collection_returns_invalid_query() {
    let mut engine = Engine::new();
    let result = engine.add_document("ghost", doc_a());
    assert!(matches!(
        result,
        Err(mini_search::Error::InvalidQuery { .. })
    ));
}

// --- Error-path: numeric filter on text field ---

#[test]
fn compare_filter_on_text_field_returns_error() {
    let (engine, _) = setup();
    let result = engine.search("meds", "notes:>5");
    assert!(matches!(
        result,
        Err(mini_search::Error::InvalidQuery { .. })
    ));
}

// --- Support: SearchMetrics ---

#[test]
fn search_metrics_reports_total_results() {
    let (engine, _) = setup();
    let (hits, metrics) = engine.search("meds", "dose_mg:>5").unwrap();
    assert_eq!(metrics.total_results, hits.len());
    assert_eq!(hits.len(), 1);
}

#[test]
fn search_metrics_for_empty_result() {
    let (engine, _) = setup();
    let (hits, metrics) = engine.search("meds", "dog dose_mg:>5").unwrap();
    assert!(hits.is_empty());
    assert_eq!(metrics.total_results, 0);
}

// --- Free text search across fields ---

#[test]
fn free_text_matches_any_text_field() {
    let mut engine = Engine::new();
    let mut fields = HashMap::new();
    fields.insert("title".to_string(), json!("dog"));
    fields.insert("notes".to_string(), json!("cat"));
    let cfgs = HashMap::from([
        ("title".to_string(), FieldConfig::new(FieldType::Text)),
        ("notes".to_string(), FieldConfig::new(FieldType::Text)),
    ]);
    engine.configure_fields("docs", cfgs);
    engine
        .add_document("docs", Document::new("d1", fields))
        .unwrap();
    let (hits, _) = engine.search("docs", "dog").unwrap();
    assert_eq!(ids(&hits), vec!["d1"]);
}

// --- Empty query returns all docs ---

#[test]
fn empty_query_returns_all_docs() {
    let (engine, _) = setup();
    let (hits, _) = engine.search("meds", "").unwrap();
    let mut result_ids: Vec<&str> = ids(&hits);
    result_ids.sort();
    assert_eq!(result_ids, vec!["d_a", "d_b"]);
}
