use hyper::Response;
use hyper::body::Bytes;
use mini_serve::{handler, QueryParams, RouteBuilder, ServeError};

async fn handle_echo_query(
    req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    let params = req.extensions().get::<QueryParams>().cloned().unwrap_or_default();
    let body = serde_json::json!(params.0);
    Ok(Response::new(mini_serve::body(Bytes::from(serde_json::to_string(&body).unwrap()))))
}

#[tokio::test]
async fn single_query_param_is_accessible() {
    let port = RouteBuilder::stateless()
        .get("/search", handler(handle_echo_query))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/search?q=hello"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["q"], "hello");
}

#[tokio::test]
async fn multiple_query_params_are_accessible() {
    let port = RouteBuilder::stateless()
        .get("/search", handler(handle_echo_query))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/search?q=hello&page=2"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["q"], "hello");
    assert_eq!(body["page"], "2");
}

#[tokio::test]
async fn missing_query_string_yields_empty_params() {
    let port = RouteBuilder::stateless()
        .get("/noquery", handler(handle_echo_query))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/noquery"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_object().unwrap().is_empty());
}

#[tokio::test]
async fn query_params_are_present_even_on_404() {
    async fn handle_404_query(
        req: hyper::Request<hyper::body::Incoming>,
        _state: mini_serve::State<()>,
    ) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
        let params = req.extensions().get::<QueryParams>().cloned().unwrap_or_default();
        let q = params.0.get("q").cloned().unwrap_or_default();
        let body = serde_json::json!({ "q": q });
        Ok(Response::new(mini_serve::body(Bytes::from(serde_json::to_string(&body).unwrap()))))
    }

    // Register a catch-all so we can inspect query params on a different path
    let port = RouteBuilder::stateless()
        .get("/custom", handler(handle_404_query))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/custom?q=test"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["q"], "test");
}
