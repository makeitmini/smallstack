use mini_search::{Comparison, ExactIndex, NumKey, NumericIndex};
use std::collections::HashSet;

fn setup_numeric() -> NumericIndex {
    let mut idx = NumericIndex::new();
    idx.insert("dose_mg", NumKey::new(1.0).unwrap(), "d_1");
    idx.insert("dose_mg", NumKey::new(5.0).unwrap(), "d_5a");
    idx.insert("dose_mg", NumKey::new(5.0).unwrap(), "d_5b");
    idx.insert("dose_mg", NumKey::new(10.0).unwrap(), "d_10");
    idx
}

fn set(ids: &[&str]) -> HashSet<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

#[test]
fn gt_excludes_bound() {
    let idx = setup_numeric();
    assert_eq!(idx.compare("dose_mg", Comparison::Gt, 5.0), set(&["d_10"]));
}

#[test]
fn gte_includes_bound() {
    let idx = setup_numeric();
    assert_eq!(
        idx.compare("dose_mg", Comparison::Gte, 5.0),
        set(&["d_5a", "d_5b", "d_10"])
    );
}

#[test]
fn lt_excludes_bound() {
    let idx = setup_numeric();
    assert_eq!(
        idx.compare("dose_mg", Comparison::Lt, 5.0),
        set(&["d_1"])
    );
}

#[test]
fn lte_includes_bound() {
    let idx = setup_numeric();
    assert_eq!(
        idx.compare("dose_mg", Comparison::Lte, 5.0),
        set(&["d_1", "d_5a", "d_5b"])
    );
}

#[test]
fn eq_matches_exact() {
    let idx = setup_numeric();
    assert_eq!(
        idx.compare("dose_mg", Comparison::Eq, 5.0),
        set(&["d_5a", "d_5b"])
    );
}

#[test]
fn range_inclusive_returns_correct_docs() {
    let idx = setup_numeric();
    assert_eq!(
        idx.range("dose_mg", 5.0, 10.0),
        set(&["d_5a", "d_5b", "d_10"])
    );
}

#[test]
fn range_empty_when_no_overlap() {
    let idx = setup_numeric();
    let result = idx.range("dose_mg", 6.0, 9.0);
    assert!(result.is_empty());
}

#[test]
fn exact_matching_returns_correct_docs() {
    let mut idx = ExactIndex::new();
    idx.insert("active", "true", "d_5a");
    idx.insert("active", "true", "d_5b");
    idx.insert("active", "false", "d_1");

    assert_eq!(
        idx.matching("active", "true"),
        set(&["d_5a", "d_5b"])
    );
    assert_eq!(idx.matching("active", "false"), set(&["d_1"]));
}

#[test]
fn exact_nonexistent_value_returns_empty() {
    let mut idx = ExactIndex::new();
    idx.insert("status", "active", "d_1");
    assert!(idx.matching("status", "inactive").is_empty());
}

#[test]
fn nonexistent_field_returns_empty() {
    let idx = NumericIndex::new();
    let result = idx.compare("ghost", Comparison::Gt, 5.0);
    assert!(result.is_empty());
}

#[test]
fn nonexistent_field_in_exact_returns_empty() {
    let idx = ExactIndex::new();
    assert!(idx.matching("ghost", "x").is_empty());
}

#[test]
fn remove_cleans_up_numeric() {
    let mut idx = setup_numeric();
    idx.remove("dose_mg", "d_5a");
    let result = idx.compare("dose_mg", Comparison::Gte, 5.0);
    assert_eq!(result, set(&["d_5b", "d_10"]));
}

#[test]
fn remove_cleans_up_exact() {
    let mut idx = ExactIndex::new();
    idx.insert("active", "true", "d_1");
    idx.insert("active", "true", "d_2");
    idx.remove("active", "d_1");
    assert_eq!(idx.matching("active", "true"), set(&["d_2"]));
}

#[test]
fn nan_bound_returns_empty() {
    let idx = setup_numeric();
    let result = idx.compare("dose_mg", Comparison::Gt, f64::NAN);
    assert!(result.is_empty());
}
