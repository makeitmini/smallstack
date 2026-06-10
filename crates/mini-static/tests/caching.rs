mod helpers {
    use tempfile::TempDir;

    pub struct ServerGuard {
        pub port: u16,
        pub _dir: TempDir,
    }

    pub async fn setup() -> ServerGuard {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("test.bin"), b"hello world").unwrap();
        let port = mini_static::Server::new(dir.path().to_path_buf())
            .run_ephemeral()
            .await
            .unwrap();
        ServerGuard { port, _dir: dir }
    }
}

#[tokio::test]
async fn static_response_carries_validators() {
    let server = helpers::setup().await;

    let resp1 = reqwest::get(format!("http://127.0.0.1:{}/test.bin", server.port))
        .await
        .expect("first request");
    assert_eq!(resp1.status(), 200);

    let etag1 = resp1
        .headers()
        .get("etag")
        .expect("etag missing")
        .to_str()
        .expect("etag not valid string")
        .to_string();
    let last_mod1 = resp1
        .headers()
        .get("last-modified")
        .expect("last-modified missing")
        .to_str()
        .expect("last-modified not valid string")
        .to_string();
    let cache_ctrl = resp1
        .headers()
        .get("cache-control")
        .expect("cache-control missing")
        .to_str()
        .expect("cache-control not valid string")
        .to_string();

    assert!(!etag1.is_empty(), "etag should not be empty");
    assert!(!last_mod1.is_empty(), "last-modified should not be empty");
    assert_eq!(cache_ctrl, "no-cache");

    let body1 = resp1.text().await.expect("body 1");
    assert_eq!(body1, "hello world");

    // Second request to same file should have identical ETag
    let resp2 = reqwest::get(format!("http://127.0.0.1:{}/test.bin", server.port))
        .await
        .expect("second request");
    assert_eq!(resp2.status(), 200);

    let etag2 = resp2
        .headers()
        .get("etag")
        .expect("etag missing on second request")
        .to_str()
        .expect("etag not valid string on second request")
        .to_string();

    assert_eq!(
        &etag1, &etag2,
        "ETag should be stable across requests to unchanged file"
    );
}

#[tokio::test]
async fn conditional_get_with_etag_returns_304() {
    let server = helpers::setup().await;

    // First request to collect ETag
    let resp1 = reqwest::get(format!("http://127.0.0.1:{}/test.bin", server.port))
        .await
        .expect("first request");
    assert_eq!(resp1.status(), 200);

    let etag = resp1
        .headers()
        .get("etag")
        .expect("etag missing")
        .to_str()
        .expect("etag not valid string")
        .to_string();

    // Second request with If-None-Match
    let client = reqwest::Client::new();
    let resp2 = client
        .get(format!("http://127.0.0.1:{}/test.bin", server.port))
        .header("if-none-match", &etag)
        .send()
        .await
        .expect("second request");

    assert_eq!(
        resp2.status(),
        304,
        "should return 304 Not Modified when ETag matches"
    );
    assert_eq!(
        resp2.content_length(),
        Some(0),
        "304 response should have empty body"
    );
}

#[tokio::test]
async fn conditional_get_with_if_modified_since_returns_304() {
    let server = helpers::setup().await;

    // First request to collect Last-Modified
    let resp1 = reqwest::get(format!("http://127.0.0.1:{}/test.bin", server.port))
        .await
        .expect("first request");
    assert_eq!(resp1.status(), 200);

    let last_modified = resp1
        .headers()
        .get("last-modified")
        .expect("last-modified missing")
        .to_str()
        .expect("last-modified not valid string")
        .to_string();

    // Second request with If-Modified-Since
    let client = reqwest::Client::new();
    let resp2 = client
        .get(format!("http://127.0.0.1:{}/test.bin", server.port))
        .header("if-modified-since", &last_modified)
        .send()
        .await
        .expect("second request");

    assert_eq!(
        resp2.status(),
        304,
        "should return 304 Not Modified when If-Modified-Since matches"
    );
}

#[tokio::test]
async fn mismatched_etag_returns_200() {
    let server = helpers::setup().await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/test.bin", server.port))
        .header("if-none-match", "W/\"wrong-etag\"")
        .send()
        .await
        .expect("request with wrong etag");

    assert_eq!(resp.status(), 200, "should return 200 with mismatched ETag");
    let body = resp.text().await.expect("body");
    assert_eq!(body, "hello world");
}
