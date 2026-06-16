use std::fs;

use mini_serve::RouteBuilder;

fn write_tree(dir: &std::path::Path) {
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("index.html"), b"<h1>hello</h1>").unwrap();
    fs::write(dir.join("sub/page.html"), b"<h1>nested</h1>").unwrap();
    fs::write(dir.join("hello.txt"), b"world").unwrap();
}

#[tokio::test]
async fn wildcard_route_serves_static_file() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path());

    let app = RouteBuilder::stateless()
        .get("/*", mini_unified::static_handler(dir.path()))
        .seal();

    let port = app.bind_ephemeral().await.unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/hello.txt"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "world");
}

#[tokio::test]
async fn api_route_takes_precedence_over_wildcard() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path());

    let app = RouteBuilder::stateless()
        .get("/api/users", mini_serve::handler(|_, _| async {
            Ok(mini_serve::json(
                hyper::StatusCode::OK,
                &serde_json::json!({"users": ["alice", "bob"]}),
            )?)
        }))
        .get("/*", mini_unified::static_handler(dir.path()))
        .seal();

    let port = app.bind_ephemeral().await.unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/api/users"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["users"][0], "alice");
    assert_eq!(body["users"][1], "bob");
}

#[tokio::test]
async fn wildcard_returns_404_for_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path());

    let app = RouteBuilder::stateless()
        .get("/*", mini_unified::static_handler(dir.path()))
        .seal();

    let port = app.bind_ephemeral().await.unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/nonexistent.txt"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn wildcard_route_serves_file_in_nested_dir() {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path());

    let app = RouteBuilder::stateless()
        .get("/*", mini_unified::static_handler(dir.path()))
        .seal();

    let port = app.bind_ephemeral().await.unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/sub/page.html"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "<h1>nested</h1>");
}
