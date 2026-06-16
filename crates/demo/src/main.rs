use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use hyper::{Request, Response, StatusCode};
use hyper::body::Incoming;
use mini_err::Error;
use mini_log::Logger;
use mini_search::{Document, Engine, FieldConfig, FieldType, Visibility};
use mini_serve::{handler, json, json_body, path_params, QueryParams, ResponseBody, RouteBuilder, ServeError, State};
use mini_unified::StaticRouteBuilderExt;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

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
        ("p5", serde_json::json!({"title": "Mechanical Keyboard - Cherry MX Blue", "description": "Full-size mechanical keyboard with Cherry MX Blue switches and RGB backlight", "category": "electronics", "tags": ["keyboard", "mechanical", "gaming", "usb"], "price": 89.99, "rating": 4.6, "in_stock": false, "reviews": 567})),
        ("p6", serde_json::json!({"title": "Yoga Mat Premium 6mm", "description": "Non-slip exercise yoga mat with carrying strap, perfect for home workouts", "category": "sports", "tags": ["yoga", "fitness", "exercise", "mat"], "price": 34.99, "rating": 4.3, "in_stock": true, "reviews": 892})),
        ("p7", serde_json::json!({"title": "Cast Iron Skillet 12-inch", "description": "Pre-seasoned cast iron frying pan, oven safe up to 500°F", "category": "home", "tags": ["cooking", "cast-iron", "kitchen", "pan"], "price": 39.99, "rating": 4.8, "in_stock": true, "reviews": 4123})),
        ("p8", serde_json::json!({"title": "Cotton T-Shirt - Classic Fit", "description": "100% organic cotton crew neck t-shirt, available in 12 colors", "category": "clothing", "tags": ["shirt", "cotton", "organic", "casual"], "price": 19.99, "rating": 4.1, "in_stock": true, "reviews": 789})),
        ("p9", serde_json::json!({"title": "Bluetooth Speaker - Waterproof", "description": "Portable IPX7 waterproof speaker with 20h playback and deep bass", "category": "electronics", "tags": ["speaker", "bluetooth", "wireless", "audio", "waterproof"], "price": 49.99, "rating": 4.4, "in_stock": true, "reviews": 1567})),
        ("p10", serde_json::json!({"title": "Running Shoes - Lightweight", "description": "Breathable mesh running shoes with responsive cushioning for daily training", "category": "sports", "tags": ["shoes", "running", "fitness", "athletic"], "price": 129.99, "rating": 4.6, "in_stock": true, "reviews": 2345})),
        ("p11", serde_json::json!({"title": "Dark Chocolate Bar 70%", "description": "Single-origin Belgian dark chocolate, 70% cocoa, smooth and rich", "category": "grocery", "tags": ["chocolate", "dark", "snack", "belgian"], "price": 4.99, "rating": 4.3, "in_stock": true, "reviews": 5678})),
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

impl AppState {
    fn new(logger: Logger) -> Self {
        let mut search = SearchEngine { engine: Engine::new(), documents: Vec::new() };
        seed_documents(&mut search);
        Self {
            inner: Arc::new(Mutex::new(AppInner { items: Vec::new(), next_id: 1 })),
            search: Arc::new(Mutex::new(search)),
            logger,
            started_at: Instant::now(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Item {
    id: u64,
    name: String,
}

#[derive(Deserialize)]
struct CreateItemInput {
    name: String,
}

#[derive(Deserialize)]
struct ItemParams {
    id: u64,
}

#[derive(Deserialize)]
struct DivideInput {
    a: f64,
    b: f64,
}

async fn health_check(
    _req: Request<Incoming>,
    state: State<AppState>,
) -> Result<Response<ResponseBody>, ServeError> {
    let uptime = state.started_at.elapsed().as_secs();
    state.logger.info("health_check")
        .field("uptime_secs", uptime)
        .emit();
    json(StatusCode::OK, &serde_json::json!({
        "status": "ok",
        "uptime_secs": uptime,
    }))
}

async fn list_items(
    _req: Request<Incoming>,
    state: State<AppState>,
) -> Result<Response<ResponseBody>, ServeError> {
    let items = state.inner.lock().unwrap();
    let count = items.items.len();
    state.logger.info("list_items")
        .field("count", count)
        .emit();
    json(StatusCode::OK, &serde_json::json!({ "items": items.items }))
}

async fn create_item(
    req: Request<Incoming>,
    state: State<AppState>,
) -> Result<Response<ResponseBody>, ServeError> {
    let input: CreateItemInput = json_body(req).await?;

    if input.name.trim().is_empty() {
        let err = Error::bad("validation", "item name cannot be empty");
        state.logger.error("create_item").err(&err).emit();
        return Err(ServeError::from(err));
    }

    let start = Instant::now();
    let item = state.inner.lock().unwrap().add_item(input.name);

    state.logger.info("create_item")
        .field("item_id", item.id)
        .field("item_name", &item.name)
        .duration(start)
        .emit();

    json(StatusCode::CREATED, &serde_json::json!({ "item": item }))
}

async fn get_item(
    req: Request<Incoming>,
    state: State<AppState>,
) -> Result<Response<ResponseBody>, ServeError> {
    let params: ItemParams = path_params(&req)?;

    let items = state.inner.lock().unwrap();
    match items.find_item(params.id) {
        Some(item) => {
            state.logger.info("get_item")
                .field("item_id", params.id)
                .emit();
            json(StatusCode::OK, &serde_json::json!({ "item": item }))
        }
        None => {
            let err = Error::gone("item", format!("item '{}' not found", params.id));
            state.logger.warn("get_item").err(&err).emit();
            Err(ServeError::from(err))
        }
    }
}

async fn divide_handler(
    req: Request<Incoming>,
    state: State<AppState>,
) -> Result<Response<ResponseBody>, ServeError> {
    let input: DivideInput = json_body(req).await?;

    if input.b == 0.0 {
        let err = Error::bad("division", "cannot divide by zero");
        state.logger.error("divide").err(&err)
            .field("a", input.a)
            .field("b", input.b)
            .emit();
        return Err(ServeError::from(err));
    }

    let result = input.a / input.b;
    state.logger.info("divide")
        .field("a", input.a)
        .field("b", input.b)
        .field("result", result)
        .emit();

    json(StatusCode::OK, &serde_json::json!({ "result": result }))
}

async fn echo_handler(
    req: Request<Incoming>,
    state: State<AppState>,
) -> Result<Response<ResponseBody>, ServeError> {
    let start = Instant::now();
    let value: serde_json::Value = json_body(req).await?;
    let kind = match &value {
        serde_json::Value::Object(_) => "object",
        serde_json::Value::Array(_) => "array",
        _ => "other",
    };
    state.logger.info("echo")
        .field("type", kind)
        .duration(start)
        .emit();
    json(StatusCode::OK, &value)
}

async fn search_handler(
    req: Request<Incoming>,
    state: State<AppState>,
) -> Result<Response<ResponseBody>, ServeError> {
    let q = req.extensions()
        .get::<QueryParams>()
        .and_then(|qp| qp.0.get("q").cloned())
        .unwrap_or_default();

    if q.is_empty() {
        return Err(ServeError::new(400, "missing 'q' parameter"));
    }

    let search = state.search.lock().unwrap();
    match search.engine.search("products", &q) {
        Ok((hits, metrics)) => {
            state.logger.info("search")
                .field("query", &q)
                .field("results", metrics.total_results)
                .emit();
            json(StatusCode::OK, &serde_json::json!({
                "hits": hits,
                "metrics": metrics,
            }))
        }
        Err(e) => {
            state.logger.warn("search")
                .field("query", &q)
                .emit();
            Err(ServeError::new(400, e.to_string()))
        }
    }
}

async fn search_explain_handler(
    req: Request<Incoming>,
    state: State<AppState>,
) -> Result<Response<ResponseBody>, ServeError> {
    let qp = req.extensions()
        .get::<QueryParams>()
        .ok_or_else(|| ServeError::new(400, "missing query parameters"))?;
    let query = qp.0.get("q").ok_or_else(|| ServeError::new(400, "missing 'q' parameter"))?;
    let doc_id = qp.0.get("id").ok_or_else(|| ServeError::new(400, "missing 'id' parameter"))?;

    let search = state.search.lock().unwrap();
    match search.engine.explain("products", query, doc_id) {
        Some(explain) => {
            state.logger.info("search_explain")
                .field("query", query)
                .field("doc_id", doc_id)
                .emit();
            json(StatusCode::OK, &serde_json::json!(explain))
        }
        None => {
            let err = Error::gone("search", format!("document '{}' not explainable for '{}'", doc_id, query));
            state.logger.warn("search_explain").err(&err).emit();
            Err(ServeError::from(err))
        }
    }
}

async fn search_seed_handler(
    _req: Request<Incoming>,
    state: State<AppState>,
) -> Result<Response<ResponseBody>, ServeError> {
    let mut search = state.search.lock().unwrap();
    seed_documents(&mut search);
    state.logger.info("search_seed")
        .field("count", search.documents.len())
        .emit();
    json(StatusCode::OK, &serde_json::json!({ "ok": true, "count": search.documents.len() }))
}

async fn search_documents_handler(
    _req: Request<Incoming>,
    state: State<AppState>,
) -> Result<Response<ResponseBody>, ServeError> {
    let search = state.search.lock().unwrap();
    json(StatusCode::OK, &serde_json::json!({
        "documents": search.documents,
        "count": search.documents.len(),
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let logger = Logger::from_env("demo");

    logger.info("server_starting")
        .field("version", env!("CARGO_PKG_VERSION"))
        .emit();

    let state = AppState::new(logger.clone());

    logger.info("search_seeded")
        .field("count", state.search.lock().unwrap().documents.len())
        .emit();

    let app = RouteBuilder::new(state)
        .get("/api/health", handler(health_check))
        .get("/api/items", handler(list_items))
        .post("/api/items", handler(create_item))
        .get("/api/items/:id", handler(get_item))
        .post("/api/divide", handler(divide_handler))
        .post("/api/echo", handler(echo_handler))
        .get("/api/search", handler(search_handler))
        .get("/api/search/explain", handler(search_explain_handler))
        .post("/api/search/seed", handler(search_seed_handler))
        .get("/api/search/documents", handler(search_documents_handler))
        .serve_static("./public")
        .seal();

    let addr: SocketAddr = "0.0.0.0:3000".parse()?;

    logger.info("server_listening")
        .field("addr", addr.to_string())
        .emit();

    let listener = TcpListener::bind(addr).await?;
    mini_serve::bind_with_os_shutdown(listener, app).await?;

    logger.info("server_stopped").emit();

    Ok(())
}
