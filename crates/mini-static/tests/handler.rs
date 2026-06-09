use std::convert::Infallible;
use std::fs;
use std::future::Future;
use std::pin::Pin;

use hyper::body::Bytes;
use hyper::Response;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};

use mini_static::{Handler, RequestInfo, ResponseBody, Server};

struct PingHandler;

impl Handler for PingHandler {
    fn handle(&self, info: RequestInfo) -> Pin<Box<dyn Future<Output = Option<Response<ResponseBody>>> + Send + '_>> {
        Box::pin(async move {
            if info.path == "/api/ping" {
                let b = BoxBody::new(Full::new(Bytes::from("pong")).map_err(|e: Infallible| match e {}));
                Some(
                    Response::builder()
                        .status(200)
                        .header("content-type", "text/plain")
                        .body(b)
                        .unwrap(),
                )
            } else {
                None
            }
        })
    }
}

struct InterceptHandler;

impl Handler for InterceptHandler {
    fn handle(&self, info: RequestInfo) -> Pin<Box<dyn Future<Output = Option<Response<ResponseBody>>> + Send + '_>> {
        Box::pin(async move {
            if info.path == "/index.html" {
                let b = BoxBody::new(Full::new(Bytes::from("handler_wins")).map_err(|e: Infallible| match e {}));
                Some(
                    Response::builder()
                        .status(200)
                        .header("content-type", "text/plain")
                        .body(b)
                        .unwrap(),
                )
            } else {
                None
            }
        })
    }
}

struct ServerGuard {
    port: u16,
    _dir: tempfile::TempDir,
}

async fn setup_with_handler(handler: impl Handler + 'static) -> ServerGuard {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("index.html"), b"<h1>hello</h1>").unwrap();
    let port = Server::new(dir.path().to_path_buf())
        .with_handler(handler)
        .run_ephemeral()
        .await
        .unwrap();
    ServerGuard { port, _dir: dir }
}

#[tokio::test]
async fn handler_returning_some_intercepts_request() {
    let g = setup_with_handler(PingHandler).await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/api/ping", g.port))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "pong");
}

#[tokio::test]
async fn handler_returning_none_falls_through_to_static() {
    let g = setup_with_handler(PingHandler).await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/", g.port))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "<h1>hello</h1>");
}

#[tokio::test]
async fn handler_checked_before_static_file() {
    let g = setup_with_handler(InterceptHandler).await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/index.html", g.port))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "handler_wins");
}

