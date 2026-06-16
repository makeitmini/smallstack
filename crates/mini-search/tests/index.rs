use mini_search::{Document, FieldConfig, FieldType, InvertedIndex, Tokenizer};
use serde_json::json;
use std::collections::HashMap;

fn make_cfgs() -> HashMap<String, FieldConfig> {
    HashMap::from([("notes".to_string(), FieldConfig::new(FieldType::Text))])
}

fn doc_notes(id: &str, text: &str) -> Document {
    let mut fields = HashMap::new();
    fields.insert("notes".to_string(), json!(text));
    Document::new(id, fields)
}

#[test]
fn postings_carry_exact_term_frequencies() {
    let mut idx = InvertedIndex::new();
    let cfgs = make_cfgs();
    let tok = Tokenizer::new();

    idx.insert(&doc_notes("d1", "dog dog cat"), &cfgs, &tok).unwrap();

    assert_eq!(idx.postings("notes", "dog"), vec![("d1".into(), 2)]);
    assert_eq!(idx.postings("notes", "cat"), vec![("d1".into(), 1)]);
}

#[test]
fn field_len_matches_token_count() {
    let mut idx = InvertedIndex::new();
    let cfgs = make_cfgs();
    let tok = Tokenizer::new();

    idx.insert(&doc_notes("d1", "dog dog cat"), &cfgs, &tok).unwrap();

    assert_eq!(idx.field_len("notes", "d1"), 3);
}

#[test]
fn multiple_docs_have_separate_postings() {
    let mut idx = InvertedIndex::new();
    let cfgs = make_cfgs();
    let tok = Tokenizer::new();

    idx.insert(&doc_notes("d_a", "dog"), &cfgs, &tok).unwrap();
    idx.insert(&doc_notes("d_b", "dog cat"), &cfgs, &tok).unwrap();

    let dog_postings = idx.postings("notes", "dog");
    assert_eq!(dog_postings.len(), 2);
    assert!(dog_postings.contains(&("d_a".into(), 1)));
    assert!(dog_postings.contains(&("d_b".into(), 1)));

    let cat_postings = idx.postings("notes", "cat");
    assert_eq!(cat_postings, vec![("d_b".into(), 1)]);
}

#[test]
fn remove_deletes_doc_and_cleans_empty_entries() {
    let mut idx = InvertedIndex::new();
    let cfgs = make_cfgs();
    let tok = Tokenizer::new();

    idx.insert(&doc_notes("d1", "dog cat"), &cfgs, &tok).unwrap();
    idx.remove("d1", "notes");

    assert_eq!(idx.postings("notes", "dog"), vec![]);
    assert_eq!(idx.postings("notes", "cat"), vec![]);
    assert_eq!(idx.field_len("notes", "d1"), 0);
}

#[test]
fn reinsert_replaces_not_duplicates() {
    let mut idx = InvertedIndex::new();
    let cfgs = make_cfgs();
    let tok = Tokenizer::new();

    idx.insert(&doc_notes("d1", "dog dog"), &cfgs, &tok).unwrap();
    idx.insert(&doc_notes("d1", "cat"), &cfgs, &tok).unwrap();

    assert_eq!(idx.postings("notes", "dog"), vec![]);
    assert_eq!(idx.postings("notes", "cat"), vec![("d1".into(), 1)]);
    assert_eq!(idx.field_len("notes", "d1"), 1);
}

#[test]
fn non_searchable_field_is_skipped() {
    let mut idx = InvertedIndex::new();
    let mut cfgs = make_cfgs();
    cfgs.get_mut("notes").unwrap().searchable = false;
    let tok = Tokenizer::new();

    idx.insert(&doc_notes("d1", "dog dog cat"), &cfgs, &tok).unwrap();

    assert_eq!(idx.postings("notes", "dog"), vec![]);
    assert_eq!(idx.field_len("notes", "d1"), 0);
}

#[test]
fn keyword_field_is_skipped_by_inverted_index() {
    let mut idx = InvertedIndex::new();
    let cfgs = HashMap::from([("status".to_string(), FieldConfig::new(FieldType::Keyword))]);
    let tok = Tokenizer::new();
    let mut fields = HashMap::new();
    fields.insert("status".to_string(), json!("active"));
    let doc = Document::new("d1", fields);

    idx.insert(&doc, &cfgs, &tok).unwrap();

    assert_eq!(idx.postings("status", "active"), vec![]);
}

#[test]
fn doc_freq_tracks_docs_containing_term() {
    let mut idx = InvertedIndex::new();
    let cfgs = make_cfgs();
    let tok = Tokenizer::new();

    idx.insert(&doc_notes("d1", "dog cat"), &cfgs, &tok).unwrap();
    idx.insert(&doc_notes("d2", "dog"), &cfgs, &tok).unwrap();
    idx.insert(&doc_notes("d3", "cat"), &cfgs, &tok).unwrap();

    assert_eq!(idx.doc_freq("notes", "dog"), 2);
    assert_eq!(idx.doc_freq("notes", "cat"), 2);
    assert_eq!(idx.doc_freq("notes", "bird"), 0);
}

#[test]
fn num_docs_and_avg_field_len() {
    let mut idx = InvertedIndex::new();
    let cfgs = make_cfgs();
    let tok = Tokenizer::new();

    idx.insert(&doc_notes("d1", "dog cat"), &cfgs, &tok).unwrap();
    idx.insert(&doc_notes("d2", "dog"), &cfgs, &tok).unwrap();

    assert_eq!(idx.num_docs("notes"), 2);
    assert!((idx.avg_field_len("notes") - 1.5).abs() < 1e-10);
}

#[test]
fn unknown_field_returns_defaults() {
    let idx = InvertedIndex::new();
    assert_eq!(idx.postings("ghost", "x"), vec![]);
    assert_eq!(idx.field_len("ghost", "d1"), 0);
    assert_eq!(idx.doc_freq("ghost", "x"), 0);
    assert_eq!(idx.num_docs("ghost"), 0);
    assert!((idx.avg_field_len("ghost") - 0.0).abs() < f64::EPSILON);
}
