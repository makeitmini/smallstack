use std::time::Duration;

use mini_static::Server;
use tokio::sync::oneshot;
use tokio::time::sleep;

#[tokio::test]
async fn shutdown_drains_inflight_and_stops_accepting() {
    // Arrange: Create a temporary directory with a file
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("file.txt"), b"hello world").unwrap();

    let srv = Server::new(dir.path().to_path_buf());
    let (tx, rx) = oneshot::channel::<()>();

    let addr: std::net::SocketAddr = "127.0.0.1:0".parse().expect("invalid addr");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind failed");
    let port = listener.local_addr().unwrap().port();
    let server_addr = format!("http://127.0.0.1:{}", port);

    let server_task = tokio::spawn(async move {
        srv.run_with_shutdown(listener, async move {
            let _ = rx.await;
        })
        .await
    });

    sleep(Duration::from_millis(50)).await;

    // Act: Start an in-flight request
    let client1 = reqwest::Client::new();
    let inflight_request = tokio::spawn({
        let addr = server_addr.clone();
        async move {
            client1
                .get(format!("{}/file.txt", addr))
                .timeout(Duration::from_secs(5))
                .send()
                .await
        }
    });

    // Give request time to reach the handler
    sleep(Duration::from_millis(50)).await;

    // Send shutdown signal while request is in-flight
    tx.send(()).expect("shutdown signal failed");

    // Give server time to process shutdown signal
    sleep(Duration::from_millis(50)).await;

    // Try to make a new connection to verify it's refused (use new client to force new connection)
    let client2 = reqwest::Client::new();
    let new_request = tokio::spawn({
        let addr = server_addr.clone();
        async move {
            client2
                .get(format!("{}/file.txt", addr))
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
            assert_eq!(response.text().await.unwrap(), "hello world");
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
