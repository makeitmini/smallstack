use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use mini_err::Error;
use mini_log::Logger;
use mini_search::{Document, Engine, FieldConfig, FieldType, Visibility};
use mini_serve::{handler, json, json_body, path_params, QueryParams, RouteBuilder};
use mini_unified::StaticRouteBuilderExt;

#[derive(Clone)]
struct AppState {
    inner: Arc<Mutex<AppInner>>,
    search: Arc<Mutex<SearchEngine>>,
    logger: Logger,
    started_at: Instant,
}

struct AppInner {
    items: Vec<Item>,
    next_id: u64,
}

impl AppInner {
    fn add_item(&mut self, name: String) -> Item {
        let id = self.next_id;
        self.next_id += 1;
        let item = Item { id, name };
        self.items.push(item.clone());
        item
    }

    fn find_item(&self, id: u64) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }
}

struct SearchEngine {
    engine: Engine,
    documents: Vec<Document>,
}

unsafe impl Send for SearchEngine {}
unsafe impl Sync for SearchEngine {}

fn seed_documents(search: &mut SearchEngine) {
    let mut cfgs = HashMap::new();
    cfgs.insert("title".into(), FieldConfig { field_type: FieldType::Text, boost: 2.0, searchable: true, visibility: Visibility::Indexed, value_boosts: HashMap::new() });
    cfgs.insert("description".into(), FieldConfig::new(FieldType::Text));
    cfgs.insert("category".into(), FieldConfig::new(FieldType::Keyword));
    cfgs.insert("tags".into(), FieldConfig::new(FieldType::Tags));
    cfgs.insert("price".into(), FieldConfig::new(FieldType::Float));
    cfgs.insert("rating".into(), FieldConfig::new(FieldType::Float));
    cfgs.insert("in_stock".into(), FieldConfig::new(FieldType::Boolean));
    cfgs.insert("reviews".into(), FieldConfig::new(FieldType::Integer));

    let mut engine = Engine::new();
    engine.configure_fields("products", cfgs);

    let mut docs = Vec::new();
    let raw: Vec<(&str, serde_json::Value)> = vec![
        ("p1", serde_json::json!({"title": "Wireless Bluetooth Headphones", "description": "Noise-cancelling over-ear headphones with 30h battery life", "category": "electronics", "tags": ["audio", "wireless", "bluetooth", "headphones"], "price": 79.99, "rating": 4.5, "in_stock": true, "reviews": 1243})),
        ("p2", serde_json::json!({"title": "USB-C Charging Cable 2m", "description": "Fast charging braided USB-C cable compatible with all modern devices", "category": "electronics", "tags": ["cable", "usb", "charging"], "price": 12.99, "rating": 4.2, "in_stock": true, "reviews": 892})),
        ("p3", serde_json::json!({"title": "Stainless Steel Water Bottle", "description": "Double-walled insulated bottle keeps drinks cold for 24h or hot for 12h", "category": "home", "tags": ["bottle", "insulated", "stainless"], "price": 24.95, "rating": 4.7, "in_stock": true, "reviews": 2156})),
        ("p4", serde_json::json!({"title": "Organic Green Tea 100 bags", "description": "Premium loose-leaf green tea from Japan, rich in antioxidants", "category": "grocery", "tags": ["tea", "organic", "green", "beverage"], "price": 8.49, "rating": 4.4, "in_stock": true, "reviews": 3451})),
        ("p12", serde_json::json!({"title": "LED Desk Lamp - Touch Control", "description": "Adjustable LED desk lamp with 5 brightness levels and USB charging port", "category": "home", "tags": ["lamp", "led", "desk", "lighting", "usb"], "price": 29.99, "rating": 4.2, "in_stock": true, "reviews": 1023})),
    ];

    for (id, fields) in &raw {
        let doc = Document::new(*id, fields.as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect());
        engine.add_document("products", doc.clone()).unwrap();
        docs.push(doc);
    }

    search.engine = engine;
    search.documents = docs;
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Item {
    id: u64,
    name: String,
}

#[derive(serde::Deserialize)]
struct CreateItemInput {
    name: String,
}

#[derive(serde::Deserialize)]
struct ItemParams {
    id: u64,
}

#[derive(serde::Deserialize)]
struct DivideInput {
    a: f64,
    b: f64,
}

async fn health_check(
    _req: hyper::Request<hyper::body::Incoming>,
    state: mini_serve::State<AppState>,
) -> Result<hyper::Response<mini_serve::ResponseBody>, mini_serve::ServeError> {
    let uptime = state.started_at.elapsed().as_secs();
    state.logger.info("health_check")
        .field("uptime_secs", uptime)
        .emit();
    json(hyper::StatusCode::OK, &serde_json::json!({
        "status": "ok",
        "uptime_secs": uptime,
    }))
}

async fn list_items_handler(
    _req: hyper::Request<hyper::body::Incoming>,
    state: mini_serve::State<AppState>,
) -> Result<hyper::Response<mini_serve::ResponseBody>, mini_serve::ServeError> {
    let items = state.inner.lock().unwrap();
    let count = items.items.len();
    state.logger.info("list_items")
        .field("count", count)
        .emit();
    json(hyper::StatusCode::OK, &serde_json::json!({ "items": items.items }))
}

async fn create_item_handler(
    req: hyper::Request<hyper::body::Incoming>,
    state: mini_serve::State<AppState>,
) -> Result<hyper::Response<mini_serve::ResponseBody>, mini_serve::ServeError> {
    let input: CreateItemInput = json_body(req).await?;

    if input.name.trim().is_empty() {
        let err = Error::bad("validation", "item name cannot be empty");
        state.logger.error("create_item").err(&err).emit();
        return Err(mini_serve::ServeError::from(err));
    }

    let item = state.inner.lock().unwrap().add_item(input.name);

    state.logger.info("create_item")
        .field("item_id", item.id)
        .field("item_name", &item.name)
        .emit();

    json(hyper::StatusCode::CREATED, &serde_json::json!({ "item": item }))
}

async fn get_item_handler(
    req: hyper::Request<hyper::body::Incoming>,
    state: mini_serve::State<AppState>,
) -> Result<hyper::Response<mini_serve::ResponseBody>, mini_serve::ServeError> {
    let params: ItemParams = path_params(&req)?;

    let items = state.inner.lock().unwrap();
    match items.find_item(params.id) {
        Some(item) => {
            state.logger.info("get_item")
                .field("item_id", params.id)
                .emit();
            json(hyper::StatusCode::OK, &serde_json::json!({ "item": item }))
        }
        None => {
            let err = Error::gone("item", format!("item '{}' not found", params.id));
            state.logger.warn("get_item").err(&err).emit();
            Err(mini_serve::ServeError::from(err))
        }
    }
}

async fn divide_handler(
    req: hyper::Request<hyper::body::Incoming>,
    state: mini_serve::State<AppState>,
) -> Result<hyper::Response<mini_serve::ResponseBody>, mini_serve::ServeError> {
    let input: DivideInput = json_body(req).await?;

    if input.b == 0.0 {
        let err = Error::bad("division", "cannot divide by zero");
        state.logger.error("divide").err(&err)
            .field("a", input.a)
            .field("b", input.b)
            .emit();
        return Err(mini_serve::ServeError::from(err));
    }

    let result = input.a / input.b;
    state.logger.info("divide")
        .field("a", input.a)
        .field("b", input.b)
        .field("result", result)
        .emit();

    json(hyper::StatusCode::OK, &serde_json::json!({ "result": result }))
}

async fn echo_handler(
    req: hyper::Request<hyper::body::Incoming>,
    state: mini_serve::State<AppState>,
) -> Result<hyper::Response<mini_serve::ResponseBody>, mini_serve::ServeError> {
    let value: serde_json::Value = json_body(req).await?;
    let kind = match &value {
        serde_json::Value::Object(_) => "object",
        serde_json::Value::Array(_) => "array",
        _ => "other",
    };
    state.logger.info("echo")
        .field("type", kind)
        .emit();
    json(hyper::StatusCode::OK, &value)
}

async fn search_handler(
    req: hyper::Request<hyper::body::Incoming>,
    state: mini_serve::State<AppState>,
) -> Result<hyper::Response<mini_serve::ResponseBody>, mini_serve::ServeError> {
    let q = req.extensions()
        .get::<QueryParams>()
        .and_then(|qp| qp.0.get("q").cloned())
        .unwrap_or_default();

    if q.is_empty() {
        return Err(mini_serve::ServeError::new(400, "missing 'q' parameter"));
    }

    let search = state.search.lock().unwrap();
    match search.engine.search("products", &q) {
        Ok((hits, metrics)) => {
            json(hyper::StatusCode::OK, &serde_json::json!({
                "hits": hits,
                "metrics": metrics,
            }))
        }
        Err(e) => {
            Err(mini_serve::ServeError::new(400, e.to_string()))
        }
    }
}

async fn search_explain_handler(
    req: hyper::Request<hyper::body::Incoming>,
    state: mini_serve::State<AppState>,
) -> Result<hyper::Response<mini_serve::ResponseBody>, mini_serve::ServeError> {
    let qp = req.extensions()
        .get::<QueryParams>()
        .ok_or_else(|| mini_serve::ServeError::new(400, "missing query parameters"))?;
    let query = qp.0.get("q").ok_or_else(|| mini_serve::ServeError::new(400, "missing 'q' parameter"))?;
    let doc_id = qp.0.get("id").ok_or_else(|| mini_serve::ServeError::new(400, "missing 'id' parameter"))?;

    let search = state.search.lock().unwrap();
    match search.engine.explain("products", query, doc_id) {
        Some(explain) => {
            json(hyper::StatusCode::OK, &serde_json::json!(explain))
        }
        None => {
            Err(mini_serve::ServeError::new(404, format!("document '{}' not found in results", doc_id)))
        }
    }
}

async fn search_seed_handler(
    _req: hyper::Request<hyper::body::Incoming>,
    state: mini_serve::State<AppState>,
) -> Result<hyper::Response<mini_serve::ResponseBody>, mini_serve::ServeError> {
    let mut search = state.search.lock().unwrap();
    seed_documents(&mut search);
    json(hyper::StatusCode::OK, &serde_json::json!({ "ok": true, "count": search.documents.len() }))
}

async fn search_documents_handler(
    _req: hyper::Request<hyper::body::Incoming>,
    state: mini_serve::State<AppState>,
) -> Result<hyper::Response<mini_serve::ResponseBody>, mini_serve::ServeError> {
    let search = state.search.lock().unwrap();
    json(hyper::StatusCode::OK, &serde_json::json!({
        "documents": search.documents,
        "count": search.documents.len(),
    }))
}

fn make_app() -> mini_serve::App<AppState> {
    let logger = Logger::new("demo_test");
    let mut search = SearchEngine { engine: Engine::new(), documents: Vec::new() };
    seed_documents(&mut search);
    let state = AppState {
        inner: Arc::new(Mutex::new(AppInner { items: Vec::new(), next_id: 1 })),
        search: Arc::new(Mutex::new(search)),
        logger,
        started_at: Instant::now(),
    };
    RouteBuilder::new(state)
        .get("/api/health", handler(health_check))
        .get("/api/items", handler(list_items_handler))
        .post("/api/items", handler(create_item_handler))
        .get("/api/items/:id", handler(get_item_handler))
        .post("/api/divide", handler(divide_handler))
        .post("/api/echo", handler(echo_handler))
        .get("/api/search", handler(search_handler))
        .get("/api/search/explain", handler(search_explain_handler))
        .post("/api/search/seed", handler(search_seed_handler))
        .get("/api/search/documents", handler(search_documents_handler))
        .serve_static("./public")
        .seal()
}

#[tokio::test]
async fn health_endpoint_returns_ok_with_uptime() {
    let port = make_app().bind_ephemeral().await.unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/api/health"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    let uptime = body["uptime_secs"].as_u64().expect("uptime_secs is a u64");
    assert!(uptime < 60, "uptime should be less than 60s in a fresh test");
}

#[tokio::test]
async fn create_item_returns_created_with_valid_name() {
    let port = make_app().bind_ephemeral().await.unwrap();
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/api/items"))
        .json(&serde_json::json!({"name": "test-item"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["item"]["name"], "test-item");
    assert!(body["item"]["id"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn create_item_rejects_empty_name() {
    let port = make_app().bind_ephemeral().await.unwrap();
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/api/items"))
        .json(&serde_json::json!({"name": ""}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["message"], "item name cannot be empty");
}

#[tokio::test]
async fn create_and_list_items_shows_all() {
    let port = make_app().bind_ephemeral().await.unwrap();
    let client = reqwest::Client::new();

    client.post(format!("http://127.0.0.1:{port}/api/items"))
        .json(&serde_json::json!({"name": "first"}))
        .send().await.unwrap();
    client.post(format!("http://127.0.0.1:{port}/api/items"))
        .json(&serde_json::json!({"name": "second"}))
        .send().await.unwrap();

    let resp = client.get(format!("http://127.0.0.1:{port}/api/items"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["name"], "first");
    assert_eq!(items[1]["name"], "second");
}

#[tokio::test]
async fn get_item_returns_item_by_id() {
    let port = make_app().bind_ephemeral().await.unwrap();
    let client = reqwest::Client::new();

    let create_resp = client
        .post(format!("http://127.0.0.1:{port}/api/items"))
        .json(&serde_json::json!({"name": "find-me"}))
        .send()
        .await
        .unwrap();
    let created: serde_json::Value = create_resp.json().await.unwrap();
    let id = created["item"]["id"].as_u64().unwrap();

    let resp = client
        .get(format!("http://127.0.0.1:{port}/api/items/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["item"]["id"], id);
    assert_eq!(body["item"]["name"], "find-me");
}

#[tokio::test]
async fn get_item_returns_404_for_missing_id() {
    let port = make_app().bind_ephemeral().await.unwrap();
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://127.0.0.1:{port}/api/items/999"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["message"], "item '999' not found");
}

#[tokio::test]
async fn divide_returns_quotient() {
    let port = make_app().bind_ephemeral().await.unwrap();
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/api/divide"))
        .json(&serde_json::json!({"a": 10.0, "b": 2.0}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!((body["result"].as_f64().unwrap() - 5.0).abs() < 1e-10);
}

#[tokio::test]
async fn divide_rejects_division_by_zero() {
    let port = make_app().bind_ephemeral().await.unwrap();
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/api/divide"))
        .json(&serde_json::json!({"a": 1.0, "b": 0.0}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["message"], "cannot divide by zero");
}

#[tokio::test]
async fn echo_returns_same_json_object() {
    let port = make_app().bind_ephemeral().await.unwrap();
    let client = reqwest::Client::new();
    let payload = serde_json::json!({"hello": "world", "nested": {"key": 42}});

    let resp = client
        .post(format!("http://127.0.0.1:{port}/api/echo"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body, payload);
}

#[tokio::test]
async fn echo_returns_same_json_array() {
    let port = make_app().bind_ephemeral().await.unwrap();
    let client = reqwest::Client::new();
    let payload = serde_json::json!([1, 2, 3]);

    let resp = client
        .post(format!("http://127.0.0.1:{port}/api/echo"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body, payload);
}

#[tokio::test]
async fn static_root_serves_index_html() {
    let port = make_app().bind_ephemeral().await.unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let text = resp.text().await.unwrap();
    assert!(text.contains("Smallstack Demo"));
    assert!(text.contains("Search Products"));
}

#[tokio::test]
async fn static_wildcard_returns_404_for_missing_file() {
    let port = make_app().bind_ephemeral().await.unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/nonexistent.html"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn search_returns_results_for_matching_query() {
    let port = make_app().bind_ephemeral().await.unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/api/search?q=wireless"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();
    assert!(!hits.is_empty(), "expected at least one hit for 'wireless'");
    assert!(hits.iter().any(|h| h["doc"]["id"] == "p1"), "expected p1 (headphones) in results");
}

#[tokio::test]
async fn search_returns_empty_for_no_match() {
    let port = make_app().bind_ephemeral().await.unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/api/search?q=zzznotfound"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();
    assert!(hits.is_empty(), "expected no hits for 'zzznotfound'");
}

#[tokio::test]
async fn search_filters_by_category_exact() {
    let port = make_app().bind_ephemeral().await.unwrap();
    let client = reqwest::Client::new();

    // Raw unencoded query (Reqwest sends : and = as-is in URL)
    let resp = client
        .get(format!("http://127.0.0.1:{port}/api/search"))
        .query(&[("q", "category:=electronics")])
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();
    assert!(!hits.is_empty(), "expected hits for category:=electronics");
    for h in hits {
        let cat = h["doc"]["category"].as_str().unwrap();
        assert_eq!(cat, "electronics", "all results must have category=electronics");
    }

    // URL-encoded query (simulates browser encodeURIComponent)
    let resp = client
        .get(format!("http://127.0.0.1:{port}/api/search?q=category%3A%3Delectronics"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let hits = body["hits"].as_array().unwrap();
    assert!(!hits.is_empty(), "expected hits for URL-encoded category%3A%3Delectronics");
    for h in hits {
        let cat = h["doc"]["category"].as_str().unwrap();
        assert_eq!(cat, "electronics", "all results must have category=electronics");
    }
}

#[tokio::test]
async fn search_rejects_missing_q() {
    let port = make_app().bind_ephemeral().await.unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/api/search"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn search_explain_returns_field_contributions() {
    let port = make_app().bind_ephemeral().await.unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/api/search/explain?q=headphones&id=p1"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let contributions = body["field_contributions"].as_array().unwrap();
    assert!(!contributions.is_empty(), "expected field contributions for 'headphones' on p1");
}

#[tokio::test]
async fn search_seed_is_idempotent() {
    let port = make_app().bind_ephemeral().await.unwrap();
    let client = reqwest::Client::new();

    let resp1 = client
        .post(format!("http://127.0.0.1:{port}/api/search/seed"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp1.status(), 200);
    let count1 = resp1.json::<serde_json::Value>().await.unwrap()["count"].as_u64().unwrap();

    let resp2 = client
        .post(format!("http://127.0.0.1:{port}/api/search/seed"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp2.status(), 200);
    let count2 = resp2.json::<serde_json::Value>().await.unwrap()["count"].as_u64().unwrap();

    assert_eq!(count1, count2, "seeding should be idempotent and return same count");
}
