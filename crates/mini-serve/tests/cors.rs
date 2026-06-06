use mini_serve::{handler, CorsConfig, RouteBuilder, ServeError};

async fn handle_hello(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<hyper::Response<mini_serve::ResponseBody>, ServeError> {
Ok(hyper::Response::new(mini_serve::body(
    hyper::body::Bytes::from("hello"),
)))
}

#[tokio::test]
async fn cors_allow_any_adds_wildcard_header() {
    let port = RouteBuilder::stateless()
        .with_cors(CorsConfig::default())
        .get("/", handler(handle_hello))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::Client::new()
        .get(format!("http://localhost:{port}/"))
        .header("origin", "https://example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("access-control-allow-origin").unwrap(),
        "*"
    );
}

#[tokio::test]
async fn cors_preflight_returns_204() {
    let port = RouteBuilder::stateless()
        .with_cors(CorsConfig::default())
        .get("/", handler(handle_hello))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::Client::new()
        .request(reqwest::Method::OPTIONS, format!("http://localhost:{port}/"))
        .header("origin", "https://example.com")
        .header("access-control-request-method", "GET")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    assert_eq!(
        resp.headers().get("access-control-allow-origin").unwrap(),
        "*"
    );
    assert!(resp.headers()
        .get("access-control-allow-methods")
        .is_some());
    assert!(resp.headers()
        .get("access-control-allow-headers")
        .is_some());
}

#[tokio::test]
async fn cors_preflight_without_origin_is_not_preflight() {
    let port = RouteBuilder::stateless()
        .with_cors(CorsConfig::default())
        .get("/", handler(handle_hello))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::Client::new()
        .request(reqwest::Method::OPTIONS, format!("http://localhost:{port}/"))
        .send()
        .await
        .unwrap();
    // OPTIONS without Origin → not preflight, no route → 405
    assert_eq!(resp.status(), 405);
}

#[tokio::test]
async fn cors_with_credentials_echos_origin() {
    let config = CorsConfig::builder()
        .allow_credentials(true)
        .build();
    let port = RouteBuilder::stateless()
        .with_cors(config)
        .get("/", handler(handle_hello))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::Client::new()
        .get(format!("http://localhost:{port}/"))
        .header("origin", "https://myapp.com")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("access-control-allow-origin").unwrap(),
        "https://myapp.com"
    );
    assert_eq!(
        resp.headers()
            .get("access-control-allow-credentials")
            .unwrap(),
        "true"
    );
}
