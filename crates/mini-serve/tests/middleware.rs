use mini_serve::{handler, middleware, RouteBuilder, ServeError};

async fn handle_hello(
    _req: hyper::Request<hyper::body::Incoming>,
    _state: mini_serve::State<()>,
) -> Result<hyper::Response<mini_serve::ResponseBody>, ServeError> {
Ok(hyper::Response::new(mini_serve::body(
    hyper::body::Bytes::from("hello"),
)))
}

#[tokio::test]
async fn middleware_adds_header_to_response() {
    let port = RouteBuilder::stateless()
        .wrap(middleware(|h| {
            let h = h.clone();
            handler(move |req, state| {
                let h = h.clone();
                async move {
                    let mut resp = h(req, state).await?;
                    resp.headers_mut()
                        .insert("x-mw", "applied".parse().unwrap());
                    Ok(resp)
                }
            })
        }))
        .get("/", handler(handle_hello))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("x-mw").unwrap(), "applied");
}

#[tokio::test]
async fn multiple_middleware_chain_in_order() {
    let port = RouteBuilder::stateless()
        .wrap(middleware(|h| {
            let h = h.clone();
            handler(move |req, state| {
                let h = h.clone();
                async move {
                    let mut resp = h(req, state).await?;
                    resp.headers_mut()
                        .insert("x-order", "first".parse().unwrap());
                    Ok(resp)
                }
            })
        }))
        .wrap(middleware(|h| {
            let h = h.clone();
            handler(move |req, state| {
                let h = h.clone();
                async move {
                    let mut resp = h(req, state).await?;
                    resp.headers_mut()
                        .insert("x-order", "second".parse().unwrap());
                    Ok(resp)
                }
            })
        }))
        .get("/", handler(handle_hello))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("x-order").unwrap(), "second");
}

#[tokio::test]
async fn middleware_can_short_circuit_with_error() {
    let port = RouteBuilder::stateless()
        .wrap(middleware(|_h| {
            handler(move |_req, _state| async move {
                Err(ServeError::new(401, "unauthorized"))
            })
        }))
        .get("/", handler(handle_hello))
        .seal()
        .bind_ephemeral()
        .await
        .unwrap();

    let resp = reqwest::get(format!("http://localhost:{port}/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}
