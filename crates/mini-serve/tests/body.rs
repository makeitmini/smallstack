use hyper::Response;
use hyper::body::Bytes;
use serde::Deserialize;
use mini_serve::{handler, json_body, RouteBuilder, ServeError};

#[derive(Deserialize)]
struct CreateUser {
    name:  String,
    email: String,
}

async fn handle_create(
    req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    let user: CreateUser = json_body(req).await?;
    let body = serde_json::json!({ "name": user.name, "email": user.email });
    Ok(Response::new(mini_serve::body(Bytes::from(serde_json::to_string(&body).unwrap()))))
}

async fn handle_echo_json(
    req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    let value: serde_json::Value = json_body(req).await?;
    Ok(Response::new(mini_serve::body(Bytes::from(serde_json::to_string(&value).unwrap()))))
}

#[tokio::test]
async fn valid_json_body_is_deserialized() {
    let port = RouteBuilder::stateless()
        .post("/users", handler(handle_create))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://localhost:{port}/users"))
        .json(&serde_json::json!({ "name": "Alice", "email": "alice@example.com" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "Alice");
    assert_eq!(body["email"], "alice@example.com");
}

#[tokio::test]
async fn invalid_json_body_returns_400() {
    let port = RouteBuilder::stateless()
        .post("/echo", handler(handle_echo_json))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://localhost:{port}/echo"))
        .body("not-json")
        .header("content-type", "application/json")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn empty_body_returns_400() {
    let port = RouteBuilder::stateless()
        .post("/echo", handler(handle_echo_json))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://localhost:{port}/echo"))
        .body("")
        .header("content-type", "application/json")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn missing_fields_returns_400() {
    let port = RouteBuilder::stateless()
        .post("/users", handler(handle_create))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://localhost:{port}/users"))
        .json(&serde_json::json!({ "name": "Bob" }))  // missing email
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 400);
}
