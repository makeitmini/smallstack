use std::time::Instant;

use mini_log::Logger;
use mini_serve::{handler, json, RouteBuilder};

#[derive(Clone)]
struct AppState {
    logger: Logger,
    started_at: Instant,
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

fn make_app() -> mini_serve::App<AppState> {
    let logger = Logger::new("demo_test");
    let state = AppState { logger, started_at: Instant::now() };
    RouteBuilder::new(state)
        .get("/api/health", handler(health_check))
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
