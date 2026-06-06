use hyper::Response;
use http_body_util::Full;
use hyper::body::Bytes;
use mini_serve::{get_header, handler, parse_cookies, RouteBuilder, ServeError};

async fn handle_echo_header(
    req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    let val = get_header(&req, "x-custom").unwrap_or("none");
    let body = serde_json::json!({ "x-custom": val });
    Ok(Response::new(Full::new(Bytes::from(serde_json::to_string(&body).unwrap()))))
}

async fn handle_echo_cookies(
    req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    let cookies = parse_cookies(&req);
    let body = serde_json::json!(cookies);
    Ok(Response::new(Full::new(Bytes::from(serde_json::to_string(&body).unwrap()))))
}

#[tokio::test]
async fn custom_header_is_accessible_in_handler() {
    let port = RouteBuilder::stateless()
        .get("/header", handler(handle_echo_header))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://localhost:{port}/header"))
        .header("x-custom", "hello")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["x-custom"], "hello");
}

#[tokio::test]
async fn missing_header_returns_none() {
    let port = RouteBuilder::stateless()
        .get("/header", handler(handle_echo_header))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/header"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["x-custom"], "none");
}

#[tokio::test]
async fn cookies_are_parsed_in_handler() {
    let port = RouteBuilder::stateless()
        .get("/cookies", handler(handle_echo_cookies))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://localhost:{port}/cookies"))
        .header("cookie", "session=abc123; theme=dark")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["session"], "abc123");
    assert_eq!(body["theme"], "dark");
}

#[tokio::test]
async fn no_cookie_header_yields_empty_object() {
    let port = RouteBuilder::stateless()
        .get("/cookies", handler(handle_echo_cookies))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/cookies"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_object().unwrap().is_empty());
}
