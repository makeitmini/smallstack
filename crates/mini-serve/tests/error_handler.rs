use hyper::body::Bytes;
use hyper::{Response, StatusCode};
use http_body_util::combinators::BoxBody;
use http_body_util::Full;
use mini_serve::{empty, handler, RouteBuilder};

#[tokio::test]
async fn custom_error_handler_is_called_for_404() {
    let port = RouteBuilder::stateless()
        .get("/exists", handler(|_, _| async { Ok(empty(StatusCode::OK)) }))
        .with_error_handler(|status, _msg| {
            Response::builder()
                .status(status)
                .header("content-type", "text/plain")
                .body(BoxBody::new(Full::new(Bytes::from(
                    format!("custom:{}", status.as_u16()),
                ))))
                .unwrap()
        })
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/missing"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    assert_eq!(resp.text().await.unwrap(), "custom:404");
}

#[tokio::test]
async fn default_error_handler_returns_json() {
    let port = RouteBuilder::stateless()
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/missing"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/json"
    );
}
