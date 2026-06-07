use hyper::{Response, StatusCode};
use hyper::body::Bytes;
use mini_serve::{body, handler, RouteBuilder, ServeError};

async fn handle_hello(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
Ok(Response::new(body(Bytes::from("hello"))))
}

async fn handle_create(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    let resp = Response::builder()
        .status(StatusCode::CREATED)
        .body(body(Bytes::from("created")))
        .unwrap();
    Ok(resp)
}

async fn handle_user(
    req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    let params = req.extensions().get::<mini_serve::PathParams>().cloned().unwrap_or_default();
    let id = params.0.get("id").cloned().unwrap_or_default();
    let body = serde_json::json!({ "id": id });
    Ok(Response::new(mini_serve::body(Bytes::from(serde_json::to_string(&body).unwrap()))))
}

async fn handle_files(
    req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    let params = req.extensions().get::<mini_serve::PathParams>().cloned().unwrap_or_default();
    let rest = params.0.get("*").cloned().unwrap_or_default();
    let body = serde_json::json!({ "path": rest });
    Ok(Response::new(mini_serve::body(Bytes::from(serde_json::to_string(&body).unwrap()))))
}

#[tokio::test]
async fn get_route_returns_200() {
    let port = RouteBuilder::stateless()
        .get("/hello", handler(handle_hello))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/hello"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "hello");
}

#[tokio::test]
async fn post_route_returns_201() {
    let port = RouteBuilder::stateless()
        .post("/create", handler(handle_create))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://localhost:{port}/create"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    assert_eq!(resp.text().await.unwrap(), "created");
}

#[tokio::test]
async fn unregistered_route_returns_404() {
    let port = RouteBuilder::stateless()
        .get("/hello", handler(handle_hello))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/nope"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn path_param_is_extracted_with_exact_value() {
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
    assert_eq!(body["id"], "42");
}

#[tokio::test]
async fn wildcard_captures_remaining_path_segments() {
    let port = RouteBuilder::stateless()
        .get("/files/*", handler(handle_files))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/files/a/b/c"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["path"], "a/b/c");
}

#[tokio::test]
async fn head_falls_back_to_get_handler() {
    let port = RouteBuilder::stateless()
        .get("/hello", handler(handle_hello))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::Client::new()
        .head(format!("http://localhost:{port}/hello"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn method_mismatch_returns_405() {
    let port = RouteBuilder::stateless()
        .get("/hello", handler(handle_hello))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://localhost:{port}/hello"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 405);
}

#[tokio::test]
async fn put_and_delete_routes_work() {
    async fn handle_put(
        _req: hyper::Request<hyper::body::Incoming>,
        _state: mini_serve::State<()>,
    ) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
        let resp = Response::builder()
            .status(StatusCode::OK)
            .body(body(Bytes::from("updated")))
            .unwrap();
        Ok(resp)
    }

    async fn handle_delete(
        _req: hyper::Request<hyper::body::Incoming>,
        _state: mini_serve::State<()>,
    ) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
        let resp = Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(body(Bytes::new()))
            .unwrap();
        Ok(resp)
    }

    let port = RouteBuilder::stateless()
        .put("/resource", handler(handle_put))
        .delete("/resource", handler(handle_delete))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .put(format!("http://localhost:{port}/resource"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "updated");

    let resp = client
        .delete(format!("http://localhost:{port}/resource"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);
}

#[tokio::test]
async fn head_on_post_only_path_returns_not_found() {
    async fn handle_post(
        _req: hyper::Request<hyper::body::Incoming>,
        _state: mini_serve::State<()>,
    ) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
        Ok(Response::new(body(Bytes::from("created"))))
    }

    let port = RouteBuilder::stateless()
        .post("/resource", handler(handle_post))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::Client::new()
        .head(format!("http://localhost:{port}/resource"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}
