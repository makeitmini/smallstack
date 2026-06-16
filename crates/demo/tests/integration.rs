use std::sync::{Arc, Mutex};
use std::time::Instant;

use mini_err::Error;
use mini_log::Logger;
use mini_serve::{handler, json, json_body, path_params, RouteBuilder};

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

fn make_app() -> mini_serve::App<AppState> {
    let logger = Logger::new("demo_test");
    let state = AppState {
        inner: Arc::new(Mutex::new(AppInner { items: Vec::new(), next_id: 1 })),
        logger,
        started_at: Instant::now(),
    };
    RouteBuilder::new(state)
        .get("/api/health", handler(health_check))
        .get("/api/items", handler(list_items_handler))
        .post("/api/items", handler(create_item_handler))
        .get("/api/items/:id", handler(get_item_handler))
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
