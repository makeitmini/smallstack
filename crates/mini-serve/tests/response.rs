use hyper::{Response, StatusCode};
use mini_serve::{empty, handler, json, redirect, Json, RouteBuilder, ServeError};

async fn handle_json(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    json(StatusCode::OK, &serde_json::json!({"msg": "ok"}))
}

async fn handle_redirect(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    Ok(redirect("/target"))
}

async fn handle_empty(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    Ok(empty(StatusCode::NO_CONTENT))
}

#[tokio::test]
async fn json_helper_returns_200_with_json_body() {
    let port = RouteBuilder::stateless()
        .get("/json", handler(handle_json))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/json"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/json"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["msg"], "ok");
}

#[tokio::test]
async fn redirect_helper_returns_302_with_location() {
    let port = RouteBuilder::stateless()
        .get("/go", handler(handle_redirect))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client
        .get(format!("http://localhost:{port}/go"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 302);
    assert_eq!(resp.headers().get("location").unwrap(), "/target");
}

#[tokio::test]
async fn empty_helper_returns_204_no_body() {
    let port = RouteBuilder::stateless()
        .get("/delete", handler(handle_empty))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/delete"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

async fn handle_json_wrapper(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    Json(serde_json::json!({"msg": "ok"})).into_response()
}

async fn handle_json_wrapper_created(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    Json(serde_json::json!({"id": 42})).into_response_with_status(StatusCode::CREATED)
}

#[tokio::test]
async fn json_wrapper_returns_200_with_json_body() {
    let port = RouteBuilder::stateless()
        .get("/", handler(handle_json_wrapper))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/json"
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["msg"], "ok");
}

#[tokio::test]
async fn json_wrapper_with_custom_status() {
    let port = RouteBuilder::stateless()
        .get("/", handler(handle_json_wrapper_created))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["id"], 42);
}
