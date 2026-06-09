#[cfg(feature = "err")]
mod err_tests {
    use std::convert::Infallible;
    use std::fs;
    use std::future::Future;
    use std::pin::Pin;

    use hyper::body::Bytes;
    use hyper::Response;
    use http_body_util::combinators::BoxBody;
    use http_body_util::{BodyExt, Full};

    use mini_static::{Handler, RequestInfo, ResponseBody, Server, StaticError};

    struct NotFoundHandler;

    impl Handler for NotFoundHandler {
        fn handle(
            &self,
            _info: RequestInfo,
        ) -> Pin<Box<dyn Future<Output = Option<Response<ResponseBody>>> + Send + '_>> {
            Box::pin(async move {
                let static_err = StaticError::NotFound("/missing.txt".into());
                let mini_err: mini_err::Error = static_err.into();
                let body = serde_json::json!({
                    "message": mini_err.message(),
                    "code": mini_err.code(),
                });
                let json = serde_json::to_string(&body).unwrap();
                let b = BoxBody::new(Full::new(Bytes::from(json)).map_err(|e: Infallible| match e {}));
                Some(
                    Response::builder()
                        .status(mini_err.code())
                        .header("content-type", "application/json")
                        .body(b)
                        .unwrap(),
                )
            })
        }
    }

    struct ServerGuard {
        port: u16,
        _dir: tempfile::TempDir,
    }

    async fn setup_with_handler(handler: impl Handler + 'static) -> ServerGuard {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("index.html"), b"ok").unwrap();
        let port = Server::new(dir.path().to_path_buf())
            .with_handler(handler)
            .run_ephemeral()
            .await
            .unwrap();
        ServerGuard { port, _dir: dir }
    }

    #[tokio::test]
    async fn static_error_converts_to_mini_err_with_matching_code_and_message() {
        let g = setup_with_handler(NotFoundHandler).await;
        let resp = reqwest::get(format!("http://127.0.0.1:{}/missing", g.port))
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);
        let body: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(body["code"], 404);
        assert!(body["message"].as_str().unwrap().contains("not found"));
    }

    #[test]
    fn static_error_traversal_converts_to_mini_err_bad() {
        let se = StaticError::Traversal("/etc/passwd".into());
        let me: mini_err::Error = se.into();
        assert_eq!(me.code(), 400);
        assert_eq!(me.scope(), "static");
        assert!(me.message().contains("traversal"));
    }

    #[test]
    fn static_error_io_converts_to_mini_err_io() {
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "test");
        let se = StaticError::Io(io);
        let me: mini_err::Error = se.into();
        assert_eq!(me.code(), 500);
        assert_eq!(me.scope(), "static");
        assert_eq!(me.message(), "test");
    }
}

#[test]
fn static_error_display_format() {
    use mini_static::StaticError;

    let e = StaticError::NotFound("/x".into());
    assert_eq!(e.to_string(), "not found: /x");

    let e = StaticError::Traversal("/etc".into());
    assert_eq!(e.to_string(), "path traversal denied: /etc");

    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing file");
    let e = StaticError::Io(io);
    assert!(e.to_string().contains("io error:"));
    assert!(e.to_string().contains("missing file"));
}

#[test]
fn static_error_status_code() {
    use mini_static::StaticError;

    let io = std::io::Error::new(std::io::ErrorKind::Other, "");
    assert_eq!(StaticError::NotFound("/".into()).status_code(), 404);
    assert_eq!(StaticError::Traversal("/".into()).status_code(), 403);
    assert_eq!(StaticError::Io(io).status_code(), 500);
}

#[test]
fn static_error_user_message() {
    use mini_static::StaticError;

    let io = std::io::Error::new(std::io::ErrorKind::Other, "");
    assert_eq!(StaticError::NotFound("/secret".into()).user_message(), "not found");
    assert_eq!(StaticError::Traversal("/etc".into()).user_message(), "path traversal denied");
    assert_eq!(StaticError::Io(io).user_message(), "internal server error");
}
