use std::fs;

use mini_static::{Server, Transform};

struct ServerGuard {
    port: u16,
    _dir: tempfile::TempDir,
}

fn write_tree(dir: &std::path::Path) {
    fs::write(dir.join("index.html"), b"<h1>hello</h1>").unwrap();
    fs::write(dir.join("styles.css"), b"body { color: red; }").unwrap();
}

async fn setup_with_transform(transform: impl Transform + 'static) -> ServerGuard {
    let dir = tempfile::tempdir().unwrap();
    write_tree(dir.path());
    let port = Server::new(dir.path().to_path_buf())
        .with_transform(transform)
        .run_ephemeral()
        .await
        .unwrap();
    ServerGuard { port, _dir: dir }
}

#[tokio::test]
async fn transform_is_applied_to_html_response() {
    let g = setup_with_transform(|_ctype: &str, mut body: Vec<u8>| {
        let mut out = b"<!-- transformed -->".to_vec();
        out.append(&mut body);
        out
    })
    .await;

    let resp = reqwest::get(format!("http://127.0.0.1:{}/index.html", g.port))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.starts_with("<!-- transformed -->"), "body: {body}");
    assert!(body.ends_with("</h1>"), "body: {body}");
}

#[tokio::test]
async fn transform_receives_correct_content_type() {
    let g = setup_with_transform(|ctype: &str, mut body: Vec<u8>| {
        let mut out = format!("ctype:{ctype}:").into_bytes();
        out.append(&mut body);
        out
    })
    .await;

    let resp = reqwest::get(format!("http://127.0.0.1:{}/index.html", g.port))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.starts_with("ctype:text/html:"), "body: {body}");
}

#[tokio::test]
async fn transform_not_applied_to_non_matching_type() {
    let g = setup_with_transform(|ctype: &str, mut body: Vec<u8>| {
        if ctype == "text/html" {
            let mut out = b"<!-- transformed -->".to_vec();
            out.append(&mut body);
            out
        } else {
            body
        }
    })
    .await;

    let resp = reqwest::get(format!("http://127.0.0.1:{}/styles.css", g.port))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "body { color: red; }");
}
