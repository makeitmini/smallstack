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
fn info_line_contains_scope_and_message() {
    let (log, buf) = test_logger();
    log.info("started").emit();
    let out = output(&buf);
    assert!(out.starts_with("["), "expected timestamp prefix, got: {out}");
    assert!(out.contains("info(test): started"), "got: {out}");
}

#[test]
fn field_appears_in_output_as_key_eq_value() {
    let (log, buf) = test_logger();
    log.info("request").field("method", "GET").emit();
    let out = output(&buf);
    assert!(out.contains("info(test): request method=GET"), "got: {out}");
}

#[test]
fn duration_field_records_elapsed_time() {
    let (log, buf) = test_logger();
    let start = std::time::Instant::now();
    std::thread::sleep(std::time::Duration::from_millis(10));
    log.info("operation").duration(start).emit();
    let out = output(&buf);
    assert!(out.contains("duration="), "expected duration= in output, got: {out}");
    let dur_ms: u128 = out
        .split("duration=")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.trim_end_matches("ms").parse().ok())
        .unwrap_or(0);
    assert!(dur_ms >= 10, "expected duration >= 10ms, got {dur_ms}ms");
}

#[test]
fn eight_fields_all_appear_in_output() {
    let (log, buf) = test_logger();
    log.info("many_fields")
        .field("a", 1)
        .field("b", 2)
        .field("c", 3)
        .field("d", 4)
        .field("e", 5)
        .field("f", 6)
        .field("g", 7)
        .field("h", 8)
        .emit();
    let out = output(&buf);
    assert!(out.contains("info(test): many_fields a=1 b=2 c=3 d=4 e=5 f=6 g=7 h=8"), "got: {out}");
}

#[test]
fn field_value_with_control_chars_is_sanitized() {
    let (log, buf) = test_logger();
    log.info("inject")
        .field("payload", "line1\nline2\r\x00done")
        .emit();
    let out = output(&buf);
    assert!(out.contains("payload=line1 line2  done"), "expected sanitized output, got: {out}");
    // The writeln! adds a trailing \n, so remove it before checking control chars.
    let body = out.trim_end_matches(|c| c == '\r' || c == '\n');
    assert!(!body.contains('\r'), "carriage return found in output body: {body:?}");
    assert!(!body.contains('\x00'), "null byte found in output body: {body:?}");
}
