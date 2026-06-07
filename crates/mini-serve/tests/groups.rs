use hyper::{Request, Response};
use hyper::body::Bytes;
use mini_serve::{body, handler, middleware, RouteBuilder, ServeError};

async fn handle_ok(
    _req: Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    Ok(Response::new(body(Bytes::from("ok"))))
}

async fn handle_created(
    _req: Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<Response<mini_serve::ResponseBody>, ServeError> {
    Ok(Response::builder()
        .status(hyper::StatusCode::CREATED)
        .body(body(Bytes::from("created")))
        .unwrap())
}

#[tokio::test]
async fn group_routes_respond_at_prefixed_path() {
    let port = RouteBuilder::stateless()
        .group("/api/v1", |g| {
            g.get("/users", handler(handle_ok))
                .post("/users", handler(handle_created))
        })
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://localhost:{port}/api/v1/users"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "ok");

    let resp = client
        .post(format!("http://localhost:{port}/api/v1/users"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    assert_eq!(resp.text().await.unwrap(), "created");
}

#[tokio::test]
async fn group_routes_do_not_respond_at_unprefixed_path() {
    let port = RouteBuilder::stateless()
        .group("/api/v1", |g| g.get("/users", handler(handle_ok)))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/users"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn group_middleware_runs_for_all_routes() {
    let group_mw = middleware(|h| {
        let h = h.clone();
        handler(move |req, state| {
            let h = h.clone();
            async move {
                let mut resp = h(req, state).await?;
                resp.headers_mut()
                    .insert("x-group", "true".parse().unwrap());
                Ok(resp)
            }
        })
    });

    let port = RouteBuilder::stateless()
        .group("/api", |g| {
            g.wrap(group_mw)
                .get("/a", handler(handle_ok))
                .post("/b", handler(handle_created))
        })
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://localhost:{port}/api/a"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("x-group").unwrap(), "true");

    let resp = client
        .post(format!("http://localhost:{port}/api/b"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    assert_eq!(resp.headers().get("x-group").unwrap(), "true");
}

#[tokio::test]
async fn group_with_parent_middleware_applies_both() {
    let parent_mw = middleware(|h| {
        let h = h.clone();
        handler(move |req, state| {
            let h = h.clone();
            async move {
                let mut resp = h(req, state).await?;
                resp.headers_mut()
                    .insert("x-parent", "applied".parse().unwrap());
                Ok(resp)
            }
        })
    });

    let group_mw = middleware(|h| {
        let h = h.clone();
        handler(move |req, state| {
            let h = h.clone();
            async move {
                let mut resp = h(req, state).await?;
                resp.headers_mut()
                    .insert("x-group", "true".parse().unwrap());
                Ok(resp)
            }
        })
    });

    let port = RouteBuilder::stateless()
        .wrap(parent_mw)
        .group("/v1", |g| g.wrap(group_mw).get("/items", handler(handle_ok)))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/v1/items"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("x-group").unwrap(), "true");
    assert_eq!(resp.headers().get("x-parent").unwrap(), "applied");
}

#[tokio::test]
async fn multiple_groups_do_not_conflict() {
    let port = RouteBuilder::stateless()
        .group("/api", |g| g.get("/a", handler(handle_ok)))
        .group("/v2", |g| g.get("/b", handler(handle_ok)))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/api/a"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = reqwest::get(format!("http://localhost:{port}/v2/b"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = reqwest::get(format!("http://localhost:{port}/api/b"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}
