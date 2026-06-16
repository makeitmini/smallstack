use crate::bounds::{BM25_B, BM25_K1};
use crate::index::InvertedIndex;
use std::collections::HashMap;

pub fn score_text(
    index: &InvertedIndex,
    field: &str,
    terms: &[String],
    boost: f32,
) -> Vec<(String, f32)> {
    let avgdl = index.avg_field_len(field) as f64;
    let n_docs = index.num_docs(field) as f64;

    if n_docs == 0.0 || avgdl == 0.0 || terms.is_empty() || boost == 0.0 {
        return vec![];
    }

    let mut scores: HashMap<String, f64> = HashMap::new();

    for term in terms {
        let n_t = index.doc_freq(field, term) as f64;
        if n_t == 0.0 {
            continue;
        }

        let idf = (1.0 + (n_docs - n_t + 0.5) / (n_t + 0.5)).ln();

        for (doc_id, tf) in index.postings(field, term) {
            let tf = tf as f64;
            let field_len = index.field_len(field, &doc_id) as f64;

            let numerator = tf * (BM25_K1 + 1.0);
            let denominator =
                tf + BM25_K1 * (1.0 - BM25_B + BM25_B * (field_len / avgdl));
            let term_score = idf * numerator / denominator;

            *scores.entry(doc_id).or_insert(0.0) += term_score;
        }
    }

    let boost = boost as f64;
    let mut result: Vec<(String, f32)> = scores
        .into_iter()
        .map(|(k, v)| (k, (v * boost) as f32))
        .collect();

    result.sort_by(|a, b| b.1.total_cmp(&a.1));
    result
}
