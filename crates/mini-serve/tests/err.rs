#[cfg(feature = "err")]
use mini_serve::{handler, RouteBuilder, ServeError};

#[cfg(feature = "err")]
async fn handle_gone(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<hyper::Response<mini_serve::ResponseBody>, ServeError> {
    Err(mini_err::Error::gone("db", "record not found"))?
}

#[cfg(feature = "err")]
#[tokio::test]
async fn mini_err_gone_maps_to_404_response() {
    let port = RouteBuilder::stateless()
        .get("/err", handler(handle_gone))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/err"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["message"], "record not found");
}
