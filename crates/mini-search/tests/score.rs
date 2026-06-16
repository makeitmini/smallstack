use mini_search::{score_text, Document, FieldConfig, FieldType, InvertedIndex, Tokenizer};
use serde_json::json;
use std::collections::HashMap;

/// Build a two-doc corpus for the worked example:
/// d_a.notes = "dog"      (len 1)
/// d_b.notes = "dog cat"  (len 2)
/// avgdl = 1.5, N = 2, n("dog") = 2
///
/// IDF("dog") = ln(1 + 0.5/2.5) = 0.182322
/// score_a = 0.182322 * (1*2.2)/(1 + 1.2*(0.25 + 0.75*(1/1.5))) = 0.21112
/// score_b = 0.182322 * (1*2.2)/(1 + 1.2*(0.25 + 0.75*(2/1.5))) = 0.16044
fn worked_example() -> (InvertedIndex, HashMap<String, FieldConfig>) {
    let cfgs = HashMap::from([("notes".to_string(), FieldConfig::new(FieldType::Text))]);
    let tok = Tokenizer::new();
    let mut idx = InvertedIndex::new();

    let mut fa = HashMap::new();
    fa.insert("notes".to_string(), json!("dog"));
    idx.insert(&Document::new("d_a", fa), &cfgs, &tok).unwrap();

    let mut fb = HashMap::new();
    fb.insert("notes".to_string(), json!("dog cat"));
    idx.insert(&Document::new("d_b", fb), &cfgs, &tok).unwrap();

    (idx, cfgs)
}

fn score_of(idx: &InvertedIndex, cfgs: &HashMap<String, FieldConfig>, doc_id: &str) -> f32 {
    let boost = cfgs.get("notes").map(|c| c.boost).unwrap_or(1.0);
    let results = score_text(idx, "notes", &["dog".to_string()], boost);
    results
        .into_iter()
        .find(|(id, _)| id == doc_id)
        .map(|(_, s)| s)
        .unwrap_or(0.0)
}

#[test]
fn bm25_scores_match_worked_example() {
    let (idx, cfgs) = worked_example();
    let s_a = score_of(&idx, &cfgs, "d_a");
    let s_b = score_of(&idx, &cfgs, "d_b");

    assert!((s_a - 0.21112).abs() < 1e-4);
    assert!((s_b - 0.16044).abs() < 1e-4);
    assert!(s_a > s_b);
}

#[test]
fn tf_saturates_sublinearly() {
    let cfgs = HashMap::from([("notes".to_string(), FieldConfig::new(FieldType::Text))]);
    let tok = Tokenizer::new();
    let mut idx = InvertedIndex::new();

    let mut f1 = HashMap::new();
    f1.insert("notes".to_string(), json!("dog"));
    idx.insert(&Document::new("d1", f1), &cfgs, &tok).unwrap();

    let mut f3 = HashMap::new();
    f3.insert("notes".to_string(), json!("dog dog dog"));
    idx.insert(&Document::new("d3", f3), &cfgs, &tok).unwrap();

    let boost = 1.0;
    let results = score_text(&idx, "notes", &["dog".to_string()], boost);
    let scores: HashMap<String, f32> = results.into_iter().collect();

    let s1 = scores.get("d1").copied().unwrap_or(0.0);
    let s3 = scores.get("d3").copied().unwrap_or(0.0);

    assert!(s3 > s1, "higher tf should score higher");
    assert!(
        (s3 - s1 * 3.0).abs() > 1e-4,
        "tf does not scale linearly: 3x tf < 3x score"
    );
    assert!(s3 < s1 * 3.0, "3x tf scores less than 3x the 1x score");
}

#[test]
fn boost_doubles_score_exactly() {
    let cfgs = HashMap::from([("notes".to_string(), FieldConfig::new(FieldType::Text))]);
    let tok = Tokenizer::new();
    let mut idx = InvertedIndex::new();

    let mut f = HashMap::new();
    f.insert("notes".to_string(), json!("dog"));
    idx.insert(&Document::new("d1", f), &cfgs, &tok).unwrap();

    let base = score_text(&idx, "notes", &["dog".to_string()], 1.0);
    let base_score = base.first().map(|(_, s)| *s).unwrap_or(0.0);

    let boosted = score_text(&idx, "notes", &["dog".to_string()], 2.0);
    let boosted_score = boosted.first().map(|(_, s)| *s).unwrap_or(0.0);

    assert!((boosted_score - base_score * 2.0).abs() < 1e-4);
}

#[test]
fn zero_boost_returns_empty() {
    let (idx, _cfgs) = worked_example();
    let results = score_text(&idx, "notes", &["dog".to_string()], 0.0);
    assert!(results.is_empty());
}

#[test]
fn unknown_term_returns_empty() {
    let (idx, cfgs) = worked_example();
    let boost = cfgs.get("notes").map(|c| c.boost).unwrap_or(1.0);
    let results = score_text(&idx, "notes", &["nonexistent".to_string()], boost);
    assert!(results.is_empty());
}

#[test]
fn unknown_field_returns_empty() {
    let (idx, _) = worked_example();
    let results = score_text(&idx, "ghost", &["dog".to_string()], 1.0);
    assert!(results.is_empty());
}

#[test]
fn scores_are_sorted_descending() {
    let (idx, cfgs) = worked_example();
    let boost = cfgs.get("notes").map(|c| c.boost).unwrap_or(1.0);
    let results = score_text(&idx, "notes", &["dog".to_string()], boost);

    for w in results.windows(2) {
        assert!(w[0].1 >= w[1].1, "scores must be sorted descending");
    }
}
