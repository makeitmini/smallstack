use hyper::Response;
use http_body_util::Full;
use hyper::body::Bytes;
use serde::Deserialize;
use mini_serve::{handler, path_params, RouteBuilder, ServeError};

#[derive(Deserialize)]
struct UserPath {
    id: u32,
}

async fn handle_user(
    req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    let path: UserPath = path_params(&req)?;
    let body = serde_json::json!({ "id": path.id });
    Ok(Response::new(Full::new(Bytes::from(serde_json::to_string(&body).unwrap()))))
}

#[tokio::test]
async fn path_param_extracts_typed_value() {
    let port = RouteBuilder::stateless()
        .get("/users/:id", handler(handle_user))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/users/42"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["id"], 42);
}

#[tokio::test]
async fn path_param_with_invalid_type_returns_400() {
    let port = RouteBuilder::stateless()
        .get("/users/:id", handler(handle_user))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/users/abc"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}
