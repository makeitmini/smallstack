use std::sync::atomic::{AtomicU32, Ordering};

use hyper::{Response, StatusCode};
use hyper::body::Bytes;
use mini_serve::{body, handler, RouteBuilder, ServeError, State};

async fn read_state(
    _req: hyper::Request<hyper::body::Incoming>,
    state: State<u32>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    let val = *state;
    let resp = Response::builder()
        .status(StatusCode::OK)
        .body(body(Bytes::from(val.to_string())))
        .unwrap();
    Ok(resp)
}

async fn increment_and_read(
    _req: hyper::Request<hyper::body::Incoming>,
    state: State<std::sync::Arc<AtomicU32>>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    let prev = state.fetch_add(1, Ordering::SeqCst);
    let resp = Response::builder()
        .status(StatusCode::OK)
        .body(body(Bytes::from(prev.to_string())))
        .unwrap();
    Ok(resp)
}

#[tokio::test]
async fn state_value_is_accessible_in_handler() {
    let port = RouteBuilder::new(42u32)
        .get("/read", handler(read_state))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/read"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "42");
}

#[tokio::test]
async fn state_is_shared_across_concurrent_requests() {
    let counter = std::sync::Arc::new(AtomicU32::new(0));
    let port = RouteBuilder::new(counter.clone())
        .get("/inc", handler(increment_and_read))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let url = format!("http://localhost:{port}/inc");

    let r1 = client.get(&url).send().await.unwrap();
    let r2 = client.get(&url).send().await.unwrap();

    // Each handler sees the pre-increment value, so responses are 0 and 1
    // (order depends on which request the server processes first)
    let mut responses = vec![r1.text().await.unwrap(), r2.text().await.unwrap()];
    responses.sort();
    assert_eq!(responses, vec!["0", "1"]);

    // Final count is 2
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn state_inner_returns_cloned_value() {
    let state = mini_serve::State::new(42u32);
    assert_eq!(state.inner(), 42);
    // Verify it's a clone, not a reference
    let cloned = state.inner();
    assert_eq!(cloned, 42);
}
