use mini_search::{Document, Engine, FieldConfig, FieldType, SearchHit, Visibility};
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
    let doc = doc.unwrap();
    assert_eq!(doc.id, "d_a");
    assert_eq!(doc.get("dose_mg"), Some(&json!(3.0)));
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

// --- Defect #2 regression: value boost applied ONCE (not × k²) ---

fn boost_setup(boost: Option<f32>) -> (Engine, HashMap<String, FieldConfig>) {
    let mut species_cfg = FieldConfig::new(FieldType::Text);
    if let Some(b) = boost {
        species_cfg.value_boosts.insert("dog".to_string(), b);
    }
    let cfgs = HashMap::from([
        ("notes".to_string(), FieldConfig::new(FieldType::Text)),
        ("species".to_string(), species_cfg),
    ]);
    let mut engine = Engine::new();
    engine.configure_fields("pets", cfgs.clone());

    let mut f1 = HashMap::new();
    f1.insert("notes".to_string(), json!("dog"));
    f1.insert("species".to_string(), json!("dog"));
    engine.add_document("pets", Document::new("d_dog", f1)).unwrap();

    let mut f2 = HashMap::new();
    f2.insert("notes".to_string(), json!("dog"));
    f2.insert("species".to_string(), json!("cat"));
    engine.add_document("pets", Document::new("d_cat", f2)).unwrap();

    (engine, cfgs)
}

#[test]
fn value_boost_applied_once() {
    let (engine, _) = boost_setup(Some(2.0));
    let (hits, _) = engine.search("pets", "dog").unwrap();
    let boosted = hits
        .iter()
        .find(|h| h.doc.id == "d_dog")
        .map(|h| h.score)
        .unwrap_or(0.0);

    let (engine_base, _) = boost_setup(None);
    let (hits_base, _) = engine_base.search("pets", "dog").unwrap();
    let base = hits_base
        .iter()
        .find(|h| h.doc.id == "d_dog")
        .map(|h| h.score)
        .unwrap_or(0.0);

    assert!((boosted - base * 2.0).abs() < 1e-4, "boosted should be base × 2");
    assert!(
        (boosted - base * 4.0).abs() > 1e-4,
        "boosted should NOT be base × 4 (double application)"
    );
}

#[test]
fn penalty_halves_score_once() {
    let (engine, _) = boost_setup(Some(0.5));
    let (hits, _) = engine.search("pets", "dog").unwrap();
    let boosted = hits
        .iter()
        .find(|h| h.doc.id == "d_dog")
        .map(|h| h.score)
        .unwrap_or(0.0);

    let (engine_base, _) = boost_setup(None);
    let (hits_base, _) = engine_base.search("pets", "dog").unwrap();
    let base = hits_base
        .iter()
        .find(|h| h.doc.id == "d_dog")
        .map(|h| h.score)
        .unwrap_or(0.0);

    assert!((boosted - base * 0.5).abs() < 1e-4, "penalty should halve the score once");
}

#[test]
fn value_boost_on_non_matching_field_leaves_score_unchanged() {
    let (engine, _) = boost_setup(Some(2.0));
    let (hits, _) = engine.search("pets", "dog").unwrap();
    let cat_score = hits
        .iter()
        .find(|h| h.doc.id == "d_cat")
        .map(|h| h.score)
        .unwrap_or(0.0);

    let (engine_base, _) = boost_setup(None);
    let (hits_base, _) = engine_base.search("pets", "dog").unwrap();
    let base_cat = hits_base
        .iter()
        .find(|h| h.doc.id == "d_cat")
        .map(|h| h.score)
        .unwrap_or(0.0);

    assert!(
        (cat_score - base_cat).abs() < 1e-4,
        "cat doc should have unchanged score (species≠dog)"
    );
}

#[test]
fn boost_and_penalty_compose_multiplicatively() {
    let mut species_cfg = FieldConfig::new(FieldType::Text);
    species_cfg.value_boosts.insert("dog".to_string(), 2.0);
    species_cfg.value_boosts.insert("cat".to_string(), 0.5);

    let cfgs = HashMap::from([
        ("notes".to_string(), FieldConfig::new(FieldType::Text)),
        ("species".to_string(), species_cfg),
    ]);
    let mut engine = Engine::new();
    engine.configure_fields("pets", cfgs);

    let mut f1 = HashMap::new();
    f1.insert("notes".to_string(), json!("dog"));
    f1.insert("species".to_string(), json!("dog"));
    engine.add_document("pets", Document::new("d1", f1)).unwrap();

    let mut f2 = HashMap::new();
    f2.insert("notes".to_string(), json!("dog"));
    f2.insert("species".to_string(), json!("cat"));
    engine.add_document("pets", Document::new("d2", f2)).unwrap();

    let (hits, _) = engine.search("pets", "dog").unwrap();
    let score_dog = hits.iter().find(|h| h.doc.id == "d1").map(|h| h.score).unwrap_or(0.0);
    let score_cat = hits.iter().find(|h| h.doc.id == "d2").map(|h| h.score).unwrap_or(0.0);

    let (engine_base, _) = boost_setup(None);
    let (hits_base, _) = engine_base.search("pets", "dog").unwrap();
    let base_dog = hits_base.iter().find(|h| h.doc.id == "d_dog").map(|h| h.score).unwrap_or(0.0);

    assert!((score_dog - base_dog * 2.0).abs() < 1e-4);
    assert!((score_cat - base_dog * 0.5).abs() < 1e_4);
}

// --- Visibility: Hidden fields are searchable but absent from results ---

#[test]
fn hidden_field_searchable_but_absent_from_search_results() {
    let mut internal_cfg = FieldConfig::new(FieldType::Text);
    internal_cfg.visibility = Visibility::Hidden;
    let cfgs = HashMap::from([
        ("brand_name".to_string(), FieldConfig::new(FieldType::Text)),
        ("internal_notes".to_string(), internal_cfg),
    ]);
    let mut engine = Engine::new();
    engine.configure_fields("meds", cfgs);

    let mut f1 = HashMap::new();
    f1.insert("brand_name".to_string(), json!("Acetaminophen"));
    f1.insert("internal_notes".to_string(), json!("confidential review pending"));
    engine.add_document("meds", Document::new("d_1", f1)).unwrap();

    let mut f2 = HashMap::new();
    f2.insert("brand_name".to_string(), json!("Ibuprofen"));
    f2.insert("internal_notes".to_string(), json!("approved"));
    engine.add_document("meds", Document::new("d_2", f2)).unwrap();

    let (hits, _) = engine.search("meds", "internal_notes:confidential").unwrap();
    assert_eq!(ids(&hits), vec!["d_1"], "searchable by hidden field");

    assert!(hits[0].doc.get("internal_notes").is_none(), "hidden field stripped");
    assert_eq!(
        hits[0].doc.get("brand_name"),
        Some(&json!("Acetaminophen")),
        "visible field preserved"
    );
}

#[test]
fn lookup_redacts_hidden_fields() {
    let mut internal_cfg = FieldConfig::new(FieldType::Text);
    internal_cfg.visibility = Visibility::Hidden;
    let cfgs = HashMap::from([
        ("brand_name".to_string(), FieldConfig::new(FieldType::Text)),
        ("internal_notes".to_string(), internal_cfg),
    ]);
    let mut engine = Engine::new();
    engine.configure_fields("meds", cfgs);

    let mut f1 = HashMap::new();
    f1.insert("brand_name".to_string(), json!("Acetaminophen"));
    f1.insert("internal_notes".to_string(), json!("confidential"));
    engine.add_document("meds", Document::new("d_1", f1)).unwrap();

    let doc = engine.lookup("meds", "d_1").unwrap();
    assert_eq!(doc.id, "d_1");
    assert_eq!(
        doc.get("brand_name"),
        Some(&json!("Acetaminophen")),
        "visible field preserved in lookup"
    );
    assert!(doc.get("internal_notes").is_none(), "hidden field stripped in lookup");
}
