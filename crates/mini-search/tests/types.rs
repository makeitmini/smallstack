use mini_search::{Document, Error, FieldConfig, FieldType, Visibility};
use serde_json::{json, Value};
use std::collections::HashMap;

#[test]
fn document_round_trips_through_json() {
    let mut fields = HashMap::new();
    fields.insert("brand_name".to_string(), json!("Metacam"));
    fields.insert("dose_mg".to_string(), json!(1.5));

    let doc = Document::new("med-1", fields);
    let json_str = serde_json::to_string(&doc).unwrap();
    let round: Document = serde_json::from_str(&json_str).unwrap();

    assert_eq!(round.id, "med-1");
    assert_eq!(round.get("dose_mg"), Some(&json!(1.5)));
    assert_eq!(round.get("brand_name"), Some(&json!("Metacam")));
}

#[test]
fn field_config_defaults() {
    let cfg = FieldConfig::new(FieldType::Text);
    assert!((cfg.boost - 1.0).abs() < f32::EPSILON);
    assert!(cfg.searchable);
    assert_eq!(cfg.visibility, Visibility::Indexed);
}

#[test]
fn field_config_round_trips_through_json() {
    let cfg = FieldConfig::new(FieldType::Float);
    let json_str = serde_json::to_string(&cfg).unwrap();
    let round: FieldConfig = serde_json::from_str(&json_str).unwrap();
    assert_eq!(round.field_type, FieldType::Float);
    assert!((round.boost - 1.0).abs() < f32::EPSILON);
}

#[test]
fn visibility_defaults_to_indexed() {
    assert_eq!(Visibility::default(), Visibility::Indexed);
}

// --- Error tests ---

#[test]
fn not_found_display_and_kind() {
    let err = Error::not_found("collection", "x");
    assert_eq!(err.to_string(), "Not found: collection with id x");
    assert_eq!(err.kind(), "not_found");
}

#[test]
fn invalid_query_display_and_kind() {
    let err = Error::invalid_query("unterminated quote");
    assert_eq!(err.to_string(), "Invalid query: unterminated quote");
    assert_eq!(err.kind(), "invalid_query");
}

#[test]
fn invalid_value_display_and_kind() {
    let err = Error::invalid_value("NaN is not allowed");
    assert_eq!(err.to_string(), "Invalid value: NaN is not allowed");
    assert_eq!(err.kind(), "invalid_value");
}

#[test]
fn store_display_and_kind() {
    let err = Error::store("corrupt blob");
    assert_eq!(err.to_string(), "Store error: corrupt blob");
    assert_eq!(err.kind(), "store");
}

#[test]
fn io_error_conversion() {
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "no file");
    let err: Error = io.into();
    assert_eq!(err.kind(), "io");
}

#[test]
fn not_found_partial_eq() {
    let a = Error::not_found("collection", "x");
    let b = Error::not_found("collection", "x");
    assert_eq!(a, b);
}

#[test]
fn different_variants_not_equal() {
    let a = Error::not_found("x", "y");
    let b = Error::invalid_query("bad");
    assert_ne!(a, b);
}
