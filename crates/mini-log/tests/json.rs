use std::io::Write;
use std::sync::{Arc, Mutex};

use mini_log::{Format, Level, Logger};

struct SharedBuf(Arc<Mutex<Vec<u8>>>);

impl Write for SharedBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

fn json_logger() -> (Logger, Arc<Mutex<Vec<u8>>>) {
    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = Arc::new(Mutex::new(
        Box::new(SharedBuf(buf.clone())) as Box<dyn Write + Send + Sync>,
    ));
    let log = Logger::new("test")
        .with_level(Level::Trace)
        .with_format(Format::Json)
        .with_writer(writer);
    (log, buf)
}

fn output(buf: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8(buf.lock().unwrap().clone()).unwrap()
}

#[test]
fn json_output_is_valid_json() {
    let (log, buf) = json_logger();
    log.info("started").emit();
    let out = output(&buf);
    let trimmed = out.trim();
    let value: serde_json::Value = serde_json::from_str(trimmed).unwrap();
    assert_eq!(value["level"], "info");
    assert_eq!(value["scope"], "test");
    assert_eq!(value["msg"], "started");
    assert!(value.get("ts").is_some());
}

#[test]
fn json_format_round_trips_through_serde_json() {
    let (log, buf) = json_logger();
    log.info("request_received")
        .field("method", "GET")
        .field("path", "/api/users")
        .field("duration", "3ms")
        .emit();
    let out = output(&buf);
    let trimmed = out.trim();
    let value: serde_json::Value = serde_json::from_str(trimmed).unwrap();
    assert_eq!(value["level"], "info");
    assert_eq!(value["scope"], "test");
    assert_eq!(value["msg"], "request_received");
    assert_eq!(value["method"], "GET");
    assert_eq!(value["path"], "/api/users");
    assert_eq!(value["duration"], "3ms");
    assert!(value.get("ts").is_some());
}
