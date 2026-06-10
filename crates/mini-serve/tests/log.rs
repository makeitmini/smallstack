#[cfg(feature = "log")]
mod log_tests {
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    use mini_serve::{handler, LoggingMiddleware, RouteBuilder};

    struct SharedBuf(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedBuf {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.0.lock().unwrap().flush()
        }
    }

    async fn handle_ok(
        _req: hyper::Request<hyper::body::Incoming>,
        _state: mini_serve::State<()>,
    ) -> Result<hyper::Response<mini_serve::ResponseBody>, mini_serve::ServeError> {
        Ok(hyper::Response::new(mini_serve::body(
            hyper::body::Bytes::from("ok"),
        )))
    }

    async fn handle_err(
        _req: hyper::Request<hyper::body::Incoming>,
        _state: mini_serve::State<()>,
    ) -> Result<hyper::Response<mini_serve::ResponseBody>, mini_serve::ServeError> {
        Err(mini_serve::ServeError::new(500, "test error"))
    }

    #[tokio::test]
    async fn logging_middleware_records_method_path_and_status() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = Arc::new(Mutex::new(
            Box::new(SharedBuf(buf.clone())) as Box<dyn Write + Send + Sync>,
        ));
        let log = mini_log::Logger::new("serve").with_writer(writer);
        let mw = LoggingMiddleware::new(log);

        let port = RouteBuilder::stateless()
            .wrap(mw.middleware())
            .get("/hello", handler(handle_ok))
            .seal()
            .bind_ephemeral()
            .await
            .unwrap();

        let resp = reqwest::get(format!("http://localhost:{port}/hello"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            out.contains("method=GET"),
            "expected method=GET in output, got: {out}"
        );
        assert!(
            out.contains("path=/hello"),
            "expected path=/hello in output, got: {out}"
        );
        assert!(
            out.contains("status=200"),
            "expected status=200 in output, got: {out}"
        );
        assert!(
            out.contains("duration="),
            "expected duration= in output, got: {out}"
        );
    }

    #[tokio::test]
    async fn logging_middleware_logs_500_at_error_level() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = Arc::new(Mutex::new(
            Box::new(SharedBuf(buf.clone())) as Box<dyn Write + Send + Sync>,
        ));
        let log = mini_log::Logger::new("serve")
            .with_level(mini_log::Level::Warn)
            .with_writer(writer);
        let mw = LoggingMiddleware::new(log);

        let port = RouteBuilder::stateless()
            .wrap(mw.middleware())
            .get("/err", handler(handle_err))
            .seal()
            .bind_ephemeral()
            .await
            .unwrap();

        let resp = reqwest::get(format!("http://localhost:{port}/err"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 500);

        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            out.contains("status=500"),
            "expected log output for 500 at warn level, got: {out}"
        );
    }

    #[tokio::test]
    async fn logging_middleware_suppresses_200_at_warn_level() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = Arc::new(Mutex::new(
            Box::new(SharedBuf(buf.clone())) as Box<dyn Write + Send + Sync>,
        ));
        let log = mini_log::Logger::new("serve")
            .with_level(mini_log::Level::Warn)
            .with_writer(writer);
        let mw = LoggingMiddleware::new(log);

        let port = RouteBuilder::stateless()
            .wrap(mw.middleware())
            .get("/hello", handler(handle_ok))
            .seal()
            .bind_ephemeral()
            .await
            .unwrap();

        let resp = reqwest::get(format!("http://localhost:{port}/hello"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            out.is_empty(),
            "expected no log output at warn level for 200, got: {out}"
        );
    }
}
