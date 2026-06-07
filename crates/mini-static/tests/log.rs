#[cfg(feature = "log")]
mod log_tests {
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    use mini_static::Server;

    struct SharedBuf(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedBuf {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.0.lock().unwrap().flush()
        }
    }

    #[tokio::test]
    async fn server_logs_file_request() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("hello.txt");
        std::fs::write(&file_path, "Hello, World!").unwrap();

        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = Arc::new(Mutex::new(
            Box::new(SharedBuf(buf.clone())) as Box<dyn Write + Send + Sync>,
        ));
        let logger = mini_log::Logger::new("serve").with_writer(writer);

        let port = Server::new(dir.path())
            .with_logger(logger)
            .run_ephemeral()
            .await
            .unwrap();

        let resp = reqwest::get(format!("http://localhost:{port}/hello.txt"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            out.contains("path=/hello.txt"),
            "expected path=/hello.txt in output, got: {out}"
        );
        assert!(
            out.contains("status=200"),
            "expected status=200 in output, got: {out}"
        );
        assert!(
            out.contains("method=GET"),
            "expected method=GET in output, got: {out}"
        );
    }
}
