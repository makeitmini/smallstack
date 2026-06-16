use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::Request;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as AutoBuilder;

fn write_tree(dir: &std::path::Path) {
    std::fs::write(dir.join("hello.txt"), b"world").unwrap();
    std::fs::write(dir.join("sub.txt"), b"nested").unwrap();
}

/// Spawn a minimal server that calls mini_static::handle_request directly,
/// proving the function works through its public API.
async fn spawn_handle_server(
    dir: Arc<std::path::PathBuf>,
) -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);
        let svc = service_fn(move |req: Request<Incoming>| {
            let dir = dir.clone();
            async move {
                let resp = mini_static::handle_request(req, &dir, &[], None).await;
                Ok::<_, Infallible>(resp)
            }
        });
        let _ = AutoBuilder::new(TokioExecutor::new())
            .serve_connection(io, svc)
            .await;
    });

    // allow server to start before client connects
    tokio::time::sleep(Duration::from_millis(50)).await;
    port
}

#[tokio::test]
async fn handle_request_returns_infallible_body_type_and_correct_content() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path());
    let dir = Arc::new(dir.path().canonicalize().unwrap());

    let port = spawn_handle_server(dir).await;

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/hello.txt"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "world");
}

#[tokio::test]
async fn handle_request_returns_404_for_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path());
    let dir = Arc::new(dir.path().canonicalize().unwrap());

    let port = spawn_handle_server(dir).await;

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/nonexistent.txt"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["message"], "not found");
}

#[tokio::test]
async fn handle_request_returns_403_for_path_traversal() {
    use tokio::io::{AsyncWriteExt as _, AsyncReadExt as _};

    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path());
    let dir = Arc::new(dir.path().canonicalize().unwrap());

    let port = spawn_handle_server(dir).await;

    let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let request = "GET /../etc/passwd HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    stream.write_all(request.as_bytes()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut buf = String::new();
    stream.read_to_string(&mut buf).await.unwrap();
    assert!(buf.starts_with("HTTP/1.1 403"), "got: {buf:?}");
    assert!(buf.contains("path traversal denied"), "got: {buf:?}");
}

#[tokio::test]
async fn handle_request_preserves_etag_and_last_modified_headers() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path());
    let dir = Arc::new(dir.path().canonicalize().unwrap());

    let port = spawn_handle_server(dir).await;

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/hello.txt"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.headers().contains_key("etag"));
    assert!(resp.headers().contains_key("last-modified"));
}
