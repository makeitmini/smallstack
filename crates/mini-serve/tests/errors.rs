use hyper::Response;
use mini_serve::{handler, RouteBuilder, ServeError};

async fn handle_bad_request(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    Err(ServeError::new(400, "bad input"))
}

async fn handle_server_error(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    Err(ServeError::new(500, "something broke"))
}

#[tokio::test]
async fn handler_returning_400_error_sends_400_to_client() {
    let port = RouteBuilder::stateless()
        .get("/bad", handler(handle_bad_request))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/bad"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn handler_returning_500_error_sends_500_to_client() {
    let port = RouteBuilder::stateless()
        .get("/break", handler(handle_server_error))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/break"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 500);
}

#[tokio::test]
async fn error_response_body_contains_message_field() {
    let port = RouteBuilder::stateless()
        .get("/bad", handler(handle_bad_request))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/bad"))
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["message"], "bad input");
}
