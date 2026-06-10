mod helpers {
    use tempfile::TempDir;

    pub struct ServerGuard {
        pub port: u16,
        pub _dir: TempDir,
    }

    pub async fn setup() -> ServerGuard {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("test.bin"), b"0123456789").unwrap();
        let port = mini_static::Server::new(dir.path().to_path_buf())
            .run_ephemeral()
            .await
            .unwrap();
        ServerGuard { port, _dir: dir }
    }
}

#[tokio::test]
async fn single_range_returns_206_with_slice() {
    let server = helpers::setup().await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/test.bin", server.port))
        .header("range", "bytes=2-5")
        .send()
        .await
        .expect("request with range");

    assert_eq!(resp.status(), 206, "should return 206 Partial Content");

    let content_range = resp
        .headers()
        .get("content-range")
        .expect("content-range missing")
        .to_str()
        .expect("content-range not valid string");
    assert_eq!(content_range, "bytes 2-5/10");

    let body = resp.text().await.expect("body");
    assert_eq!(body, "2345");
}

#[tokio::test]
async fn unsatisfiable_range_returns_416() {
    let server = helpers::setup().await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/test.bin", server.port))
        .header("range", "bytes=20-30")
        .send()
        .await
        .expect("request with unsatisfiable range");

    assert_eq!(
        resp.status(),
        416,
        "should return 416 Range Not Satisfiable for out-of-bounds range"
    );

    let content_range = resp
        .headers()
        .get("content-range")
        .expect("content-range missing")
        .to_str()
        .expect("content-range not valid string");
    assert_eq!(content_range, "bytes */10");
}

#[tokio::test]
async fn multipart_range_returns_416() {
    let server = helpers::setup().await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/test.bin", server.port))
        .header("range", "bytes=0-2,5-7")
        .send()
        .await
        .expect("request with multipart range");

    assert_eq!(
        resp.status(),
        416,
        "should return 416 for multipart ranges (not supported)"
    );

    let content_range = resp
        .headers()
        .get("content-range")
        .expect("content-range missing")
        .to_str()
        .expect("content-range not valid string");
    assert_eq!(content_range, "bytes */10");
}

#[tokio::test]
async fn range_from_start_open_end() {
    let server = helpers::setup().await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/test.bin", server.port))
        .header("range", "bytes=7-")
        .send()
        .await
        .expect("request with open-end range");

    assert_eq!(resp.status(), 206);
    let content_range = resp
        .headers()
        .get("content-range")
        .expect("content-range missing")
        .to_str()
        .expect("content-range not valid string");
    assert_eq!(content_range, "bytes 7-9/10");

    let body = resp.text().await.expect("body");
    assert_eq!(body, "789");
}

#[tokio::test]
async fn range_suffix_last_n_bytes() {
    let server = helpers::setup().await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{}/test.bin", server.port))
        .header("range", "bytes=-3")
        .send()
        .await
        .expect("request with suffix range");

    assert_eq!(resp.status(), 206);
    let content_range = resp
        .headers()
        .get("content-range")
        .expect("content-range missing")
        .to_str()
        .expect("content-range not valid string");
    assert_eq!(content_range, "bytes 7-9/10");

    let body = resp.text().await.expect("body");
    assert_eq!(body, "789");
}

#[tokio::test]
async fn full_200_response_includes_accept_ranges() {
    let server = helpers::setup().await;

    let resp = reqwest::get(format!("http://127.0.0.1:{}/test.bin", server.port))
        .await
        .expect("full request");

    assert_eq!(resp.status(), 200);
    let accept_ranges = resp
        .headers()
        .get("accept-ranges")
        .expect("accept-ranges missing")
        .to_str()
        .expect("accept-ranges not valid string");
    assert_eq!(accept_ranges, "bytes");
}
