use std::time::Duration;

use hyper::{Request, Response, StatusCode};
use hyper::body::Incoming;
use mini_serve::{handler, ResponseBody, RouteBuilder, ServeError, State};
use tokio::sync::oneshot;
use tokio::time::sleep;

async fn slow_handler(
    _req: Request<Incoming>,
    _state: State<()>,
) -> Result<Response<ResponseBody>, ServeError> {
    // Simulate a long-running operation (1 second)
    sleep(Duration::from_millis(1000)).await;
    mini_serve::json(StatusCode::OK, &serde_json::json!({ "ok": true }))
}

async fn quick_handler(
    _req: Request<Incoming>,
    _state: State<()>,
) -> Result<Response<ResponseBody>, ServeError> {
    mini_serve::json(StatusCode::OK, &serde_json::json!({ "ok": true }))
}

#[tokio::test]
async fn shutdown_drains_inflight_and_stops_accepting() {
    // Arrange: Create app with slow and quick handlers
    let app = RouteBuilder::stateless()
        .get("/slow", handler(slow_handler))
        .get("/quick", handler(quick_handler))
        .seal();

    let (tx, rx) = oneshot::channel::<()>();

    let addr: std::net::SocketAddr = "127.0.0.1:0".parse().expect("invalid addr");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind failed");
    let port = listener.local_addr().unwrap().port();
    let server_addr = format!("http://127.0.0.1:{}", port);

    let server_task = tokio::spawn(async move {
        mini_serve::bind_with_shutdown(listener, app, async move {
            let _ = rx.await;
        })
            .await
    });

    sleep(Duration::from_millis(50)).await;

    // Act: Start an in-flight request (will take 1 second)
    let client = reqwest::Client::new();
    let inflight_request = tokio::spawn({
        let addr = server_addr.clone();
        let client = client.clone();
        async move {
            client
                .get(format!("{}/slow", addr))
                .timeout(Duration::from_secs(5))
                .send()
                .await
        }
    });

    // Give request time to reach the handler
    sleep(Duration::from_millis(50)).await;

    // Send shutdown signal while request is in-flight
    tx.send(()).expect("shutdown signal failed");

    // Immediately try to make a new connection to verify it's refused
    let new_request = tokio::spawn({
        let addr = server_addr.clone();
        let client = client.clone();
        async move {
            client
                .get(format!("{}/quick", addr))
                .timeout(Duration::from_millis(200))
                .send()
                .await
        }
    });

    // Assert: In-flight request completes successfully
    let inflight_result = inflight_request.await.expect("inflight task failed");
    match inflight_result {
        Ok(response) => {
            assert_eq!(
                response.status(),
                200,
                "in-flight request must complete with 200"
            );
        }
        Err(e) => panic!("in-flight request failed: {}", e),
    }

    // Assert: New connection attempt is refused (times out or errors)
    let new_result = new_request.await.expect("new request task failed");
    let new_was_refused = matches!(new_result, Err(_));
    assert!(new_was_refused, "new connection should be refused during shutdown");

    // Assert: Server exits gracefully
    let server_result = server_task.await.expect("server task failed");
    server_result.expect("server should shut down without error");

    // Assert: Port is released after shutdown
    sleep(Duration::from_millis(50)).await;
    let port_available = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .is_ok();
    assert!(port_available, "port must be released after shutdown");
}
