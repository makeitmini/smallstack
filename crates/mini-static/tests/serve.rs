use std::fs;

use mini_static::Server;

fn write_tree(dir: &std::path::Path) {
    fs::create_dir_all(dir.join("sub/nested")).unwrap();
    fs::write(dir.join("index.html"), b"<h1>hello</h1>").unwrap();
    fs::write(dir.join("sub/index.html"), b"<h1>sub</h1>").unwrap();
    fs::write(dir.join("sub/nested/index.html"), b"<h1>nested</h1>").unwrap();
    fs::write(dir.join("sub/file.txt"), b"hello world").unwrap();
    fs::create_dir(dir.join("empty")).unwrap();
    fs::write(dir.join("sub/data.json"), b"{\"x\":1}").unwrap();
    fs::write(dir.join("styles.css"), b"body { color: red; }").unwrap();
    fs::write(dir.join("app.js"), b"const x = 1;").unwrap();
    fs::write(dir.join("image.png"), b"PNG").unwrap();
    fs::write(dir.join("graphic.svg"), b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
    fs::write(dir.join("font.woff2"), b"wOFF2").unwrap();
    fs::write(dir.join("favicon.ico"), b"ICO").unwrap();
}

struct ServerGuard {
    port: u16,
    _dir: tempfile::TempDir,
}

async fn setup() -> ServerGuard {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path());
    let srv = Server::new(dir.path().to_path_buf());
    let port = srv.run_ephemeral().await.unwrap();
    ServerGuard {
        port,
        _dir: dir,
    }
}

#[tokio::test]
async fn serves_index_html_for_root() {
    let g = setup().await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/", g.port))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "<h1>hello</h1>");
}

#[tokio::test]
async fn directory_with_index_serves_index_html() {
    let g = setup().await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/sub", g.port))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "<h1>sub</h1>");
}

#[tokio::test]
async fn nested_directory_with_index_serves_correctly() {
    let g = setup().await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/sub/nested", g.port))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "<h1>nested</h1>");
}

#[tokio::test]
async fn serves_named_file() {
    let g = setup().await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/sub/file.txt", g.port))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "hello world");
}

#[tokio::test]
async fn returns_correct_mime_type_html() {
    let g = setup().await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/index.html", g.port))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "text/html"
    );
}

#[tokio::test]
async fn returns_correct_mime_type_json() {
    let g = setup().await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/sub/data.json", g.port))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/json"
    );
}

#[tokio::test]
async fn returns_correct_mime_type_txt() {
    let g = setup().await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/sub/file.txt", g.port))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "text/plain"
    );
}

#[tokio::test]
async fn returns_correct_mime_type_css() {
    let g = setup().await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/styles.css", g.port))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "text/css"
    );
}

#[tokio::test]
async fn returns_correct_mime_type_js() {
    let g = setup().await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/app.js", g.port))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "text/javascript"
    );
}

#[tokio::test]
async fn returns_correct_mime_type_png() {
    let g = setup().await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/image.png", g.port))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "image/png"
    );
}

#[tokio::test]
async fn returns_correct_mime_type_svg() {
    let g = setup().await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/graphic.svg", g.port))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "image/svg+xml"
    );
}

#[tokio::test]
async fn returns_correct_mime_type_woff2() {
    let g = setup().await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/font.woff2", g.port))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "font/woff2"
    );
}

#[tokio::test]
async fn returns_correct_mime_type_ico() {
    let g = setup().await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/favicon.ico", g.port))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "image/x-icon"
    );
}

#[tokio::test]
async fn missing_file_returns_404() {
    let g = setup().await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/nonexistent.html", g.port))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["message"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn path_traversal_returns_403() {
    use tokio::io::{AsyncWriteExt as _, AsyncReadExt as _};
    let g = setup().await;
    let mut stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", g.port))
        .await
        .unwrap();
    let request = "GET /../etc/passwd HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    stream.write_all(request.as_bytes()).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let mut buf = String::new();
    stream.read_to_string(&mut buf).await.unwrap();
    assert!(buf.starts_with("HTTP/1.1 403"), "got: {buf:?}");
    assert!(buf.contains("path traversal denied"), "got: {buf:?}");
}

#[tokio::test]
async fn directory_without_index_returns_404() {
    let g = setup().await;
    let resp = reqwest::get(format!("http://127.0.0.1:{}/empty", g.port))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}
