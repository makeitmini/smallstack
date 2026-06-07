#[cfg(feature = "err")]
mod err_tests {
    use std::fs;
    use std::future::Future;
    use std::pin::Pin;

    use hyper::body::Bytes;
    use hyper::Response;
    use http_body_util::combinators::BoxBody;
    use http_body_util::Full;

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
                Some(
                    Response::builder()
                        .status(mini_err.code())
                        .header("content-type", "application/json")
                        .body(BoxBody::new(Full::new(Bytes::from(json))))
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
}
