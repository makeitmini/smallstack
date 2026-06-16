# mini-search

A minimal, client-side search engine for Rust applications with BM25 scoring, filter indexes, field-level visibility, and a query language — all without external search infrastructure.

```toml
[dependencies]
mini-search = "0.14"

# Optional: disk persistence
mini-search = { version = "0.14", features = ["persist"] }

# Optional: WASM bindings (JsEngine)
mini-search = { version = "0.14", features = ["wasm"] }
```

---

## Philosophy

Most search solutions either run as a separate service (Elasticsearch, Meilisearch, Typesense) or pull in a large indexing library (Tantivy, Sonic). Mini-search does neither — it is an in-process, pure-Rust search engine that indexes documents in memory and answers queries immediately.

### Why not Tantivy / Meilisearch / Elasticsearch / SQLite FTS?

Those tools are excellent for production-scale search workloads. Mini-search is designed for the **local search** use case — the kind where you have a few thousand documents, want ranked results with fielded queries and filters, and don't want to run a separate daemon or manage index files.

- **No external process.** Everything runs in your application's memory. There is no daemon to start, no HTTP API to call, no cluster to configure.
- **No index files (by default).** Indexes are rebuilt in memory on every `Engine::open()`. Persistence is optional and explicitly gated behind the `persist` feature.
- **Predictable query semantics.** BM25 ranking with configurable field boosts, value boosts, and filter intersection. No learning-to-rank, no vector search, no typo-tolerance.
- **A single-threaded query model.** Search runs on the calling thread. There is no thread pool, no async runtime requirement, and no Rayon dependency.
- **Field-level visibility.** Index a field for searching but redact it from results. Useful for internal notes, PII, or access control.

### Design tenets

1. **Documents are JSON objects.** A document is a `HashMap<String, Value>` with a string `id`. There is no schema — field types are declared at query time via `FieldConfig`.
2. **BM25 is the ranking function.** The standard Okapi BM25 with k1=1.2 and b=0.75. Term frequency saturates sub-linearly, and document length normalization prevents short documents from dominating.
3. **Query parsing is built-in.** The query language supports free text, fielded terms (`field:term`), quoted phrases, numeric comparisons (`field:>5`, `field:[1 TO 10]`), and exact filters (`field:=value`). Queries are bounded (max 1024 bytes, 32 terms).
4. **Processing is extensible.** The `Processor` trait lets you intercept and modify queries and results — for logging, access control, or enrichment — without modifying search internals.
5. **Persistence is optional.** Serialize the document store to `state.json` behind the `persist` feature. Indexes are rebuilt on load.

---

## Usage

### Field configuration

Before indexing, declare the fields you want to search:

```rust
use mini_search::{Engine, FieldConfig, FieldType, Visibility};
use std::collections::HashMap;

let mut engine = Engine::new();

let mut cfgs = HashMap::new();
cfgs.insert("title".to_string(), FieldConfig::new(FieldType::Text));
cfgs.insert("description".to_string(), FieldConfig::new(FieldType::Text));
cfgs.insert("price".to_string(), FieldConfig::new(FieldType::Float));

engine.configure_fields("products", cfgs);
```

| FieldType     | Index           | Query operators                            |
|---------------|-----------------|--------------------------------------------|
| `Text`        | Inverted index  | Free text, fielded text, quoted phrases    |
| `TextArray`   | Inverted index  | Free text, fielded text, quoted phrases    |
| `Keyword`     | Exact index     | `:=value`                                  |
| `Tags`        | Exact index     | `:=value` (multiple values supported)      |
| `Boolean`     | Exact index     | `:=true`, `:=false`                        |
| `Integer`     | Numeric index   | `:>`, `:>=`, `:<`, `:<=`, `:[a TO b]`     |
| `Float`       | Numeric index   | `:>`, `:>=`, `:<`, `:<=`, `:[a TO b]`     |
| `Date`        | Numeric index   | `:>`, `:>=`, `:<`, `:<=`, `:[a TO b]`     |

### Adding documents

```rust
use mini_search::Document;
use serde_json::json;

let mut fields = HashMap::new();
fields.insert("title".to_string(), json!("Wireless Headphones"));
fields.insert("description".to_string(), json!("Noise cancelling with 30h battery"));
fields.insert("price".to_string(), json!(89.99));

engine.add_document("products", Document::new("prod-1", fields)).unwrap();
```

### Searching

```rust
let (hits, metrics) = engine.search("products", "wireless noise cancelling").unwrap();
println!("Found {} results", metrics.total_results);

for hit in &hits {
    println!("{} (score: {})", hit.doc.id, hit.score);
}
```

The query language supports:

| Syntax                  | Meaning                                    |
|-------------------------|--------------------------------------------|
| `hello world`           | Free text across all text fields           |
| `title:hello`           | Fielded text term                          |
| `"hello world"`         | Quoted phrase (single token)               |
| `price:>50`             | Numeric greater-than                       |
| `price:>=50`            | Numeric greater-than-or-equal              |
| `price:<100`            | Numeric less-than                          |
| `price:<=100`           | Numeric less-than-or-equal                 |
| `status:=active`        | Exact match (keyword, boolean, tags)       |
| `price:[50 TO 100]`     | Numeric range (inclusive)                  |
| `active :=true`         | Boolean exact filter                       |
| `hello status:=active`  | Free text + filter (intersection)          |

Parse a query without running it:

```rust
use mini_search::Query;

let query = Query::parse("title:dog price:>5").unwrap();
assert_eq!(query.text.len(), 1);     // one text clause
assert_eq!(query.filters.len(), 1);  // one filter
```

### BM25 scoring

```rust
use mini_search::score_text;

let inv = engine.inverted.get("products").unwrap();
let scores = score_text(inv, "title", &["wireless"], 1.0);
// Vec<(DocId, Score)> — raw BM25 scores for one field + term
```

Scoring supports per-field boosts in `FieldConfig` and per-document value boosts.

### Value boosts

Boost (or penalize) specific field values at the document level:

```rust
let mut cfg = FieldConfig::new(FieldType::Text);
cfg.value_boosts.insert("premium".to_string(), 2.0);
cfg.value_boosts.insert("budget".to_string(), 0.5);
```

A document whose `product_tier` field equals `"premium"` gets its final score multiplied by 2.0. A `"budget"` document gets multiplied by 0.5. Multiple boosts compound multiplicatively.

### Lookup

Retrieve a document by ID with hidden fields redacted:

```rust
if let Some(doc) = engine.lookup("products", "prod-1") {
    println!("Found: {}", doc.id);
}
```

### Hidden fields

Mark a field `searchable` but invisible in results:

```rust
let mut internal = FieldConfig::new(FieldType::Text);
internal.visibility = Visibility::Hidden;

// The field is indexed and searchable...
let (hits, _) = engine.search("products", "internal_notes:confidential").unwrap();
// ...but absent from every returned document
assert!(hits[0].doc.get("internal_notes").is_none());
```

| Visibility | Indexed? | In results? |
|------------|----------|-------------|
| `Indexed`  | Yes      | Yes         |
| `Stored`   | Yes      | Yes         |
| `Hidden`   | Yes      | No          |

### Field contributions

Every `SearchHit` carries a per-field, per-term scoring breakdown:

```rust
for hit in &hits {
    for c in &hit.field_contributions {
        println!("  field={}, term={}, score={}",
            c.field_name, c.term, c.score_component);
    }
}
```

### Explain

Get a scoring breakdown for a single document:

```rust
if let Some(ex) = engine.explain("products", "wireless headphones", "prod-1") {
    println!("Score: {}, source: {}", ex.score, ex.source_collection);
    for c in &ex.field_contributions {
        println!("  {} / {} = {}", c.field_name, c.term, c.score_component);
    }
}
```

Returns `None` if the document or collection does not exist.

### Multi-index search

Search across multiple collections and merge results:

```rust
let (hits, metrics) = engine.search_multi(&["products", "reviews"], "wireless").unwrap();
// Results are deduplicated by document ID (highest score wins) and sorted.
```

### Pipeline processors

Intercept and modify queries or results:

```rust
use mini_search::{Processor, SearchHit, Result};

struct AuditProcessor;

impl Processor for AuditProcessor {
    fn pre_search(&self, query: &str) -> Result<String> {
        println!("Query: {query}");
        Ok(query.to_string())
    }

    fn post_search(&self, hits: Vec<SearchHit>) -> Result<Vec<SearchHit>> {
        println!("Results: {}", hits.len());
        Ok(hits)
    }
}

engine.add_processor(AuditProcessor);
```

A processor can reject a query (return `Err` from `pre_search`) or strip results (return empty vec from `post_search`). Processors compose in registration order.

---

## Persistence (optional)

Enable the `persist` feature for disk persistence:

```toml
mini-search = { version = "0.14", features = ["persist"] }
```

```rust
use mini_search::Engine;

// Open or create a store
let mut engine = Engine::open("./data").unwrap();
engine.configure_fields("docs", cfgs);
engine.add_document("docs", doc).unwrap();
engine.save().unwrap();

// Later — reload
let engine = Engine::open("./data").unwrap();
```

- Stores `state.json` in the directory.
- Path traversal via `..` in collection names is rejected on save.
- A version field prevents loading state files from newer versions of mini-search.
- Corrupt or missing `state.json` returns an error (or an empty engine for fresh directories).

---

## WASM bindings (optional)

Enable the `wasm` feature for JavaScript interop:

```toml
mini-search = { version = "0.14", features = ["wasm"] }
```

```rust,ignore
use mini_search::JsEngine;

let mut engine = JsEngine::new();
engine.configure_fields("docs", r#"{"title":{"field_type":"Text"}}"#);

engine.add_document_json("docs", r#"{"id":"d1","title":"hello world"}"#).unwrap();

let result = engine.search_json("docs", "hello").unwrap();
// Returns JSON: {"hits":[{"doc":{"id":"d1","title":"hello world"},"score":...}],"metrics":{"total_results":1}}
```

Input and output are JSON strings. `lookup_json` returns `"null"` for missing documents. Panics are logged to the browser console via `console_error_panic_hook`.

---

## Architecture

```
Engine
  ├── documents:     HashMap<Collection, HashMap<DocId, Document>>
  ├── field_configs: HashMap<Collection, HashMap<FieldName, FieldConfig>>
  ├── inverted:      HashMap<Collection, InvertedIndex>    ← text fields
  ├── numeric:       HashMap<Collection, NumericIndex>     ← int/float/date fields
  ├── exact:         HashMap<Collection, ExactIndex>       ← keyword/boolean/tags
  ├── tokenizer:     Tokenizer                             ← lowercase, punctuation-strip, stopwords
  └── pipeline:      Vec<Box<dyn Processor>>               ← pre/post hooks

Search flow:
  query_str
    → pre_search pipeline (Processor::pre_search)
    → Query::parse (free text + filters)
    → text_candidates ∩ filter_candidates
    → BM25 scoring (per-field boost → value boost → aggregate)
    → sort + truncate (max 100 results)
    → post_search pipeline (Processor::post_search)
    → (Vec<SearchHit>, SearchMetrics)
```

---

## Comparison

| Feature                         | mini-search | Tantivy | Meilisearch | SQLite FTS |
|---------------------------------|-------------|---------|-------------|------------|
| No external process             | ✓           |         |             | ✓          |
| In-process (embedding)          | ✓           | ✓       |             | ✓          |
| BM25 ranking                    | ✓           | ✓       |             |            |
| Field-level boosts              | ✓           | ✓       | ✓           |            |
| Value-level boosts              | ✓           |         |             |            |
| Numeric filters / range queries | ✓           | ✓       | ✓           |            |
| Filter intersection             | ✓           | ✓       | ✓           | ✓          |
| Quoted phrase search            | ✓           | ✓       | ✓           | ✓          |
| Field-level visibility          | ✓           | ✓       | ✓           |            |
| Persistence (optional)          | ✓           | ✓       | ✓           | ✓          |
| WASM support (optional)         | ✓           |         |             |            |
| Pipeline processors             | ✓           |         |             |            |
| Typo-tolerance                  |             | ✓       | ✓           |            |
| Faceted search                  |             | ✓       | ✓           |            |
| Index-on-disk                   |             | ✓       | ✓           | ✓          |
| Async / multi-threaded          |             | ✓       | ✓           |            |

Mini-search is best suited for client-side, embedded, or single-tenant search use cases where you want BM25 relevance with filters and field-level control — but don't want to install, configure, or maintain a search server. If you need typo-tolerance, faceted aggregation, or large-scale indexing, consider Tantivy or Meilisearch.

---

## MSRV

The minimum supported Rust version is **1.70**. Bumping the MSRV is considered a breaking change.
