use mini_search::{Comparison, Filter, Query, TextClause};

fn free(term: &str) -> TextClause {
    TextClause {
        field: None,
        term: term.to_string(),
    }
}

#[test]
fn free_text_query() {
    let q = Query::parse("dog").unwrap();
    assert_eq!(q, Query::text("dog"));
}

#[test]
fn fielded_text_query() {
    let q = Query::parse("species:dog").unwrap();
    assert_eq!(q, Query::fielded("species", "dog"));
}

#[test]
fn compare_filter_and_free_text() {
    let q = Query::parse("dose_mg:>5 dog").unwrap();
    assert_eq!(
        q,
        Query {
            text: vec![free("dog")],
            filters: vec![Filter::cmp("dose_mg", Comparison::Gt, 5.0)],
        }
    );
}

#[test]
fn range_filter() {
    let q = Query::parse("dose_mg:[1 TO 10]").unwrap();
    assert_eq!(
        q,
        Query {
            text: vec![],
            filters: vec![Filter::range("dose_mg", 1.0, 10.0)],
        }
    );
}

#[test]
fn boolean_exact_filter() {
    let q = Query::parse("active:true").unwrap();
    assert_eq!(
        q,
        Query {
            text: vec![],
            filters: vec![Filter::eq("active", "true")],
        }
    );
}

#[test]
fn quoted_phrase() {
    let q = Query::parse("\"dog cat\" food").unwrap();
    assert_eq!(
        q,
        Query {
            text: vec![
                TextClause {
                    field: None,
                    term: "dog cat".to_string(),
                },
                free("food"),
            ],
            filters: vec![],
        }
    );
}

#[test]
fn multiple_filters() {
    let q = Query::parse("dose_mg:>5 active:true").unwrap();
    assert_eq!(
        q,
        Query {
            text: vec![],
            filters: vec![
                Filter::cmp("dose_mg", Comparison::Gt, 5.0),
                Filter::eq("active", "true"),
            ],
        }
    );
}

#[test]
fn lowercases_text_terms() {
    let q = Query::parse("Dog Cat").unwrap();
    assert_eq!(
        q,
        Query {
            text: vec![free("dog"), free("cat")],
            filters: vec![],
        }
    );
}

#[test]
fn lte_and_gte_operators() {
    let q = Query::parse("age:<=10 age:>=5").unwrap();
    assert_eq!(
        q,
        Query {
            text: vec![],
            filters: vec![
                Filter::cmp("age", Comparison::Lte, 10.0),
                Filter::cmp("age", Comparison::Gte, 5.0),
            ],
        }
    );
}

#[test]
fn eq_operator() {
    let q = Query::parse("price:=100").unwrap();
    assert_eq!(
        q,
        Query {
            text: vec![],
            filters: vec![Filter::cmp("price", Comparison::Eq, 100.0)],
        }
    );
}

#[test]
fn unterminated_quote_consumes_remainder() {
    let q = Query::parse("\"dog cat").unwrap();
    assert_eq!(q, Query::text("dog cat"));
}

// --- Error-path tests ---

#[test]
fn empty_field_value_returns_error() {
    let result = Query::parse("dose_mg:>");
    assert!(matches!(result, Err(mini_search::Error::InvalidQuery { .. })));
}

#[test]
fn invalid_range_syntax_returns_error() {
    let result = Query::parse("dose_mg:[1 TO]");
    assert!(matches!(result, Err(mini_search::Error::InvalidQuery { .. })));
}

#[test]
fn query_exceeds_max_bytes_returns_error() {
    let big = "a".repeat(1025);
    let result = Query::parse(&big);
    assert!(matches!(result, Err(mini_search::Error::InvalidQuery { .. })));
}

#[test]
fn query_exceeds_max_terms_returns_error() {
    let many = (0..33).map(|i| format!("term{}", i)).collect::<Vec<_>>().join(" ");
    let result = Query::parse(&many);
    assert!(matches!(result, Err(mini_search::Error::InvalidQuery { .. })));
}

#[test]
fn empty_string_returns_empty_query() {
    let q = Query::parse("").unwrap();
    assert!(q.text.is_empty());
    assert!(q.filters.is_empty());
}
