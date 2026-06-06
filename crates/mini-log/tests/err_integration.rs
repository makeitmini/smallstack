#![cfg(feature = "err")]

use std::io::Write;
use std::sync::{Arc, Mutex};

use mini_log::{Level, Logger};

struct SharedBuf(Arc<Mutex<Vec<u8>>>);

impl Write for SharedBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

fn test_logger() -> (Logger, Arc<Mutex<Vec<u8>>>) {
    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = Arc::new(Mutex::new(
        Box::new(SharedBuf(buf.clone())) as Box<dyn Write + Send + Sync>,
    ));
    let log = Logger::new("test").with_level(Level::Trace).with_writer(writer);
    (log, buf)
}

fn output(buf: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8(buf.lock().unwrap().clone()).unwrap()
}

#[test]
fn err_field_contains_scope_kind_and_code() {
    let (log, buf) = test_logger();
    let err = mini_err::Error::bad("parse", "missing field");

    log.error("operation_failed").err(&err).emit();

    let out = output(&buf);
    assert!(out.contains("err_scope=parse"));
    assert!(out.contains("err_kind=bad"));
    assert!(out.contains("err_code=400"));
    // msg is the dynamic message, so check it in one assertion
    assert!(out.contains("err_msg=missing field"));
}

#[test]
fn err_with_gone_variant_shows_404() {
    let (log, buf) = test_logger();
    let err = mini_err::Error::gone("db", "record not found");

    log.warn("query_failed").err(&err).emit();

    let out = output(&buf);
    assert!(out.contains("err_scope=db"));
    assert!(out.contains("err_kind=gone"));
    assert!(out.contains("err_code=404"));
    assert!(out.contains("err_msg=record not found"));
}
