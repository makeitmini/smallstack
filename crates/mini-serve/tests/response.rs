use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;

use hyper::{Response, StatusCode};
use mini_serve::{empty, handler, json, redirect, sse_stream, Json, RouteBuilder, ServeError};

async fn handle_json(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    json(StatusCode::OK, &serde_json::json!({"msg": "ok"}))
}

async fn handle_redirect(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    Ok(redirect("/target"))
}

async fn handle_empty(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    Ok(empty(StatusCode::NO_CONTENT))
}

#[tokio::test]
async fn json_helper_returns_200_with_json_body() {
    let port = RouteBuilder::stateless()
        .get("/json", handler(handle_json))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/json"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/json"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["msg"], "ok");
}

#[tokio::test]
async fn redirect_helper_returns_302_with_location() {
    let port = RouteBuilder::stateless()
        .get("/go", handler(handle_redirect))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://localhost:{port}/go"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 302);
    assert_eq!(resp.headers().get("location").unwrap(), "/target");
}

#[tokio::test]
async fn empty_helper_returns_204_no_body() {
    let port = RouteBuilder::stateless()
        .get("/delete", handler(handle_empty))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/delete"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

async fn handle_json_wrapper(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    Json(serde_json::json!({"msg": "ok"})).into_response()
}

async fn handle_json_wrapper_created(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    Json(serde_json::json!({"id": 42})).into_response_with_status(StatusCode::CREATED)
}

#[tokio::test]
async fn json_wrapper_returns_200_with_json_body() {
    let port = RouteBuilder::stateless()
        .get("/", handler(handle_json_wrapper))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/json"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["msg"], "ok");
}

#[tokio::test]
async fn json_wrapper_with_custom_status() {
    let port = RouteBuilder::stateless()
        .get("/", handler(handle_json_wrapper_created))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["id"], 42);
}

async fn handle_sse(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    Ok(sse_stream(IterStream(
        vec![
            "event a".to_string(),
            "event b".to_string(),
            "event c".to_string(),
        ],
        0,
    )))
}

struct IterStream(Vec<String>, usize);

impl Stream for IterStream {
    type Item = String;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.1 < self.0.len() {
            let item = self.0[self.1].clone();
            self.1 += 1;
            Poll::Ready(Some(item))
        } else {
            Poll::Ready(None)
        }
    }
}

#[tokio::test]
async fn sse_stream_returns_text_event_stream_content_type() {
    let port = RouteBuilder::stateless()
        .get("/events", handler(handle_sse))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/events"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
}

#[tokio::test]
async fn sse_stream_contains_all_events_in_order() {
    let port = RouteBuilder::stateless()
        .get("/events", handler(handle_sse))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let body = reqwest::get(format!("http://localhost:{port}/events"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert_eq!(body, "data: event a\n\ndata: event b\n\ndata: event c\n\n");
}

use tokio::time::{interval, Duration, Interval};

/// Stream that yields one event per `interval`, up to `total` events.
/// The first tick is immediate (tokio::time::interval contract).
struct TimedSseStream {
    count: usize,
    total: usize,
    ticker: Interval,
}

impl TimedSseStream {
    fn new(total: usize, interval_dur: Duration) -> Self {
        Self { count: 0, total, ticker: interval(interval_dur) }
    }
}

impl Stream for TimedSseStream {
    type Item = String;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.count >= self.total {
            return Poll::Ready(None);
        }
        match self.ticker.poll_tick(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(_) => {
                self.count += 1;
                Poll::Ready(Some(format!("event {}", self.count)))
            }
        }
    }
}

async fn handle_sse_timed(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<hyper::Response<mini_serve::ResponseBody>, ServeError> {
    Ok(sse_stream(TimedSseStream::new(12, Duration::from_millis(200))))
}

#[tokio::test]
async fn sse_stream_not_killed_by_header_read_timeout() {
    let port = RouteBuilder::stateless()
        .with_header_read_timeout(Duration::from_millis(500))
        .get("/events", handler(handle_sse_timed))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/events"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body.matches("event ").count(), 12, "expected 12 events over 2.4s, got body: {body:?}");
}
