use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use hyper::{Request, Response, StatusCode};
use hyper::body::Incoming;
use mini_err::Error;
use mini_log::Logger;
use mini_serve::{handler, json, json_body, path_params, ResponseBody, RouteBuilder, ServeError, State};
use mini_unified::StaticRouteBuilderExt;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

#[derive(Clone)]
struct AppState {
    inner: Arc<Mutex<AppInner>>,
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

impl AppState {
    fn new(logger: Logger) -> Self {
        Self {
            inner: Arc::new(Mutex::new(AppInner { items: Vec::new(), next_id: 1 })),
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let logger = Logger::from_env("demo");

    logger.info("server_starting")
        .field("version", env!("CARGO_PKG_VERSION"))
        .emit();

    let state = AppState::new(logger.clone());

    let app = RouteBuilder::new(state)
        .get("/api/health", handler(health_check))
        .get("/api/items", handler(list_items))
        .post("/api/items", handler(create_item))
        .get("/api/items/:id", handler(get_item))
        .post("/api/divide", handler(divide_handler))
        .post("/api/echo", handler(echo_handler))
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
