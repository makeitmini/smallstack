use std::io::Write;
use std::sync::{Arc, Mutex};

use mini_log::Logger;

struct SharedBuf(Arc<Mutex<Vec<u8>>>);

impl Write for SharedBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

#[test]
fn cloned_logger_shares_writer() {
    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = Arc::new(Mutex::new(
        Box::new(SharedBuf(buf.clone())) as Box<dyn Write + Send + Sync>,
    ));
    let log_a = Logger::new("test").with_writer(writer);
    let log_b = log_a.clone();

    log_a.info("first").emit();
    log_b.info("second").emit();

    let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
    assert!(
        out.contains("first"),
        "expected 'first' in output, got: {out}"
    );
    assert!(
        out.contains("second"),
        "expected 'second' in output, got: {out}"
    );
}

#[test]
fn clone_uses_same_level() {
    let log_a = Logger::new("test");
    let log_b = log_a.clone();
    assert_eq!(log_a.level, log_b.level);
}

#[test]
fn clone_uses_same_scope() {
    let log_a = Logger::new("my-app");
    let log_b = log_a.clone();
    assert_eq!(log_a.scope, log_b.scope);
}
