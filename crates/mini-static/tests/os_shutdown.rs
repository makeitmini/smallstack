use mini_static::Server;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn run_with_os_shutdown_accepts_listener() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("file.txt"), b"hello").unwrap();

    let srv = Server::new(dir.path().to_path_buf());
    let addr: std::net::SocketAddr = "127.0.0.1:0".parse().expect("invalid addr");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind failed");
    let port = listener.local_addr().unwrap().port();

    // Spawn server that will run until test completes
    let _server_task = tokio::spawn(async move {
        // The server will listen for signals, but we'll just let it run
        // In a real scenario this would listen for SIGTERM/SIGINT
        let _ = srv.run_with_os_shutdown(listener).await;
    });

    sleep(Duration::from_millis(100)).await;

    // Verify server is running by making a request
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/file.txt", port))
        .timeout(Duration::from_secs(5))
        .send()
        .await;

    assert!(resp.is_ok(), "server should accept requests");
    let text = resp.unwrap().text().await.unwrap();
    assert_eq!(text, "hello");
}
