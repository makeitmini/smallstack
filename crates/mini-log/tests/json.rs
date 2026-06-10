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
fn json_duration_appears_as_field() {
    let (log, buf) = json_logger();
    let start = std::time::Instant::now();
    std::thread::sleep(std::time::Duration::from_millis(10));
    log.info("timed_op").duration(start).emit();
    let out = output(&buf);
    let trimmed = out.trim();
    let value: serde_json::Value = serde_json::from_str(trimmed).unwrap();
    let dur = value["duration"].as_str().unwrap_or("");
    assert!(dur.ends_with("ms"), "expected duration ending in 'ms', got {dur}");
    let ms: u128 = dur.trim_end_matches("ms").parse().unwrap_or(0);
    assert!(ms >= 10, "expected duration >= 10ms, got {ms}ms");
}

#[test]
fn json_timestamp_has_sub_second_precision() {
    let (log, buf) = json_logger();
    log.info("first").emit();
    std::thread::sleep(std::time::Duration::from_millis(5));
    log.info("second").emit();
    let out = output(&buf);
    let lines: Vec<&str> = out.trim().lines().collect();
    assert_eq!(lines.len(), 2);

    let ts1: u64 = serde_json::from_str::<serde_json::Value>(lines[0])
        .unwrap()["ts"].as_u64().unwrap();
    let ts2: u64 = serde_json::from_str::<serde_json::Value>(lines[1])
        .unwrap()["ts"].as_u64().unwrap();

    assert!(
        ts1 > 1_700_000_000_000,
        "ts {ts1} looks like seconds (not millis)"
    );
    assert!(
        ts2 > ts1,
        "expected ts2 ({ts2}) > ts1 ({ts1}) — sub-second precision missing"
    );
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
