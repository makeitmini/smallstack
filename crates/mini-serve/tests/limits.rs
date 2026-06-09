use std::time::Duration;

use hyper::StatusCode;
use mini_serve::{empty, handler, RouteBuilder};

#[tokio::test]
async fn query_string_beyond_limit_returns_400() {
    let port = RouteBuilder::stateless()
        .get("/", handler(|_, _| async { Ok(empty(StatusCode::OK)) }))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let huge_query = "a=b&".repeat(2000);
    let resp = reqwest::get(format!("http://127.0.0.1:{port}/?{huge_query}"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn path_beyond_limit_returns_400() {
    let port = RouteBuilder::stateless()
        .get("/", handler(|_, _| async { Ok(empty(StatusCode::OK)) }))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let long_path = "/".to_string() + &"a".repeat(8200);
    let resp = reqwest::get(format!("http://127.0.0.1:{port}{long_path}"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn idle_connection_is_closed_after_timeout() {
    let port = RouteBuilder::stateless()
        .get("/", handler(|_, _| async { Ok(empty(StatusCode::OK)) }))
        .with_header_read_timeout(Duration::from_millis(100))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(250)).await;

    let _ = stream.readable().await;
    let mut buf = [0u8; 1];
    let n = stream.try_read(&mut buf).unwrap_or(0);
    assert_eq!(n, 0, "server must have closed the connection");
}
