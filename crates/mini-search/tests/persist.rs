#![cfg(feature = "persist")]

use mini_search::{Document, Engine, FieldConfig, FieldType, SearchHit};
use serde_json::json;
use std::collections::HashMap;

fn cfgs() -> HashMap<String, FieldConfig> {
    HashMap::from([
        ("notes".to_string(), FieldConfig::new(FieldType::Text)),
        ("dose_mg".to_string(), FieldConfig::new(FieldType::Float)),
    ])
}

fn ids(hits: &[SearchHit]) -> Vec<&str> {
    hits.iter().map(|h| h.doc.id.as_str()).collect()
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

#[test]
fn round_trip_reproduces_search_results() {
    let dir = tempfile::tempdir().unwrap();

    {
        let mut e = Engine::open(dir.path()).unwrap();
        e.configure_fields("meds", cfgs());
        e.add_document("meds", doc_a()).unwrap();
        e.add_document("meds", doc_b()).unwrap();
        e.save().unwrap();
    }

    let e2 = Engine::open(dir.path()).unwrap();
    let (hits, _) = e2.search("meds", "cat dose_mg:>5").unwrap();
    assert_eq!(ids(&hits), vec!["d_b"]);
}

#[test]
fn round_trip_with_range_only_query() {
    let dir = tempfile::tempdir().unwrap();

    {
        let mut e = Engine::open(dir.path()).unwrap();
        e.configure_fields("meds", cfgs());
        e.add_document("meds", doc_a()).unwrap();
        e.add_document("meds", doc_b()).unwrap();
        e.save().unwrap();
    }

    let e2 = Engine::open(dir.path()).unwrap();
    let (hits, _) = e2.search("meds", "dose_mg:>5").unwrap();
    assert_eq!(ids(&hits), vec!["d_b"]);
}

#[test]
fn version_too_new_returns_store_error() {
    let dir = tempfile::tempdir().unwrap();
    // Write a state file with version > 1
    let bad_state = r#"{"version":99,"documents":{},"field_configs":{}}"#;
    std::fs::write(dir.path().join("state.json"), bad_state).unwrap();

    let result = Engine::open(dir.path());
    assert!(matches!(result, Err(mini_search::Error::Store { .. })));
}

#[test]
fn corrupt_blob_returns_store_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("state.json"), "not valid json").unwrap();

    let result = Engine::open(dir.path());
    assert!(matches!(result, Err(mini_search::Error::Store { .. })));
}

#[test]
fn collection_name_with_dot_dot_returns_invalid_query() {
    let dir = tempfile::tempdir().unwrap();
    let bad_cfgs = HashMap::from([
        ("../../etc".to_string(), FieldConfig::new(FieldType::Text)),
    ]);

    let mut e = Engine::open(dir.path()).unwrap();
    e.configure_fields("safe", cfgs());
    e.configure_fields("../../etc", bad_cfgs);
    e.add_document("safe", doc_a()).unwrap();

    let result = e.save();
    assert!(matches!(
        result,
        Err(mini_search::Error::InvalidQuery { .. })
    ));
}

#[test]
fn round_trip_empty_engine() {
    let dir = tempfile::tempdir().unwrap();

    {
        let e = Engine::open(dir.path()).unwrap();
        e.save().unwrap();
    }

    // After empty save, we can still use the engine
    let mut e = Engine::open(dir.path()).unwrap();
    e.configure_fields("meds", cfgs());
    e.add_document("meds", doc_a()).unwrap();
    let (hits, _) = e.search("meds", "dog").unwrap();
    assert_eq!(ids(&hits), vec!["d_a"], "engine is functional after empty save");
}

#[test]
fn no_path_traversal_on_save() {
    let dir = tempfile::tempdir().unwrap();
    let mut e = Engine::open(dir.path()).unwrap();
    e.configure_fields("safe", cfgs());
    e.add_document("safe", doc_a()).unwrap();
    e.save().unwrap();

    // Only state.json should exist in dir
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].file_name(), "state.json");
}
