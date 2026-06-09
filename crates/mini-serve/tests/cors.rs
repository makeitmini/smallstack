use hyper::Method;
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
    let config = CorsConfig::builder()
        .allow_origin("*")
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
    let config = CorsConfig::builder()
        .allow_origin("*")
        .allow_method(Method::GET)
        .allow_header("*")
        .build();
    let port = RouteBuilder::stateless()
        .with_cors(config)
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
    let config = CorsConfig::builder()
        .allow_origin("*")
        .build();
    let port = RouteBuilder::stateless()
        .with_cors(config)
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
async fn cors_with_credentials_echos_specific_origin() {
    let config = CorsConfig::builder()
        .allow_origin("https://myapp.com")
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

#[tokio::test]
async fn cors_specific_origin_is_echoed() {
    let config = CorsConfig::builder()
        .allow_origin("https://myapp.com")
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
}

#[tokio::test]
async fn cors_non_matching_origin_omits_header() {
    let config = CorsConfig::builder()
        .allow_origin("https://myapp.com")
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
        .header("origin", "https://evil.com")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.headers().get("access-control-allow-origin").is_none());
}

#[tokio::test]
async fn cors_preflight_with_explicit_headers_and_max_age() {
    let config = CorsConfig::builder()
        .allow_origin("https://myapp.com")
        .allow_method(Method::GET)
        .allow_header("X-Custom")
        .max_age_secs(3600)
        .build();
    let port = RouteBuilder::stateless()
        .with_cors(config)
        .get("/", handler(handle_hello))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::Client::new()
        .request(reqwest::Method::OPTIONS, format!("http://localhost:{port}/"))
        .header("origin", "https://myapp.com")
        .header("access-control-request-method", "GET")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
    assert_eq!(
        resp.headers().get("access-control-allow-methods").unwrap(),
        "GET"
    );
    assert_eq!(
        resp.headers().get("access-control-allow-headers").unwrap(),
        "X-Custom"
    );
    assert_eq!(
        resp.headers().get("access-control-max-age").unwrap(),
        "3600"
    );
}

#[tokio::test]
async fn cors_expose_headers_appear_in_response() {
    let config = CorsConfig::builder()
        .allow_origin("*")
        .expose_header("X-Result")
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
        .header("origin", "https://example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("access-control-expose-headers").unwrap(),
        "X-Result"
    );
}

#[test]
#[should_panic(expected = "CORS misconfiguration")]
fn wildcard_origin_with_credentials_panics_in_debug() {
    CorsConfig::builder()
        .allow_origin("*")
        .allow_credentials(true)
        .build();
}

#[test]
fn specific_origin_with_credentials_is_valid() {
    let _ = CorsConfig::builder()
        .allow_origin("https://app.example.com")
        .allow_credentials(true)
        .build();
}

#[test]
fn wildcard_origin_without_credentials_is_valid() {
    let _ = CorsConfig::builder()
        .allow_origin("*")
        .build();
}

#[tokio::test]
async fn specific_origin_cors_response_has_vary_origin() {
    let config = CorsConfig::builder()
        .allow_origin("https://myapp.com")
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
        resp.headers().get("vary").unwrap(),
        "origin"
    );
}

#[tokio::test]
async fn wildcard_origin_cors_response_does_not_have_vary_origin() {
    let config = CorsConfig::builder()
        .allow_origin("*")
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
        .header("origin", "https://example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert!(resp.headers().get("vary").is_none());
}

async fn handle_with_vary(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<hyper::Response<mini_serve::ResponseBody>, ServeError> {
    let mut resp = hyper::Response::new(mini_serve::body(
        hyper::body::Bytes::from("cached"),
    ));
    resp.headers_mut().insert(
        hyper::header::VARY,
        hyper::header::HeaderValue::from_static("accept-encoding"),
    );
    Ok(resp)
}

#[tokio::test]
async fn cors_vary_merges_with_existing_vary_header() {
    let config = CorsConfig::builder()
        .allow_origin("https://myapp.com")
        .build();
    let port = RouteBuilder::stateless()
        .with_cors(config)
        .get("/", handler(handle_with_vary))
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
    let vary = resp.headers().get("vary").unwrap().to_str().unwrap();
    assert!(vary.contains("origin"), "should contain origin, got: {vary}");
    assert!(vary.contains("accept-encoding"), "should contain accept-encoding, got: {vary}");
}
