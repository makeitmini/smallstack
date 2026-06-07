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

fn silent_logger(level: Level) -> (Logger, Arc<Mutex<Vec<u8>>>) {
    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = Arc::new(Mutex::new(
        Box::new(SharedBuf(buf.clone())) as Box<dyn Write + Send + Sync>,
    ));
    let log = Logger::new("test").with_level(level).with_writer(writer);
    (log, buf)
}

fn output(buf: &Arc<Mutex<Vec<u8>>>) -> String {
    String::from_utf8(buf.lock().unwrap().clone()).unwrap()
}

#[test]
fn debug_suppressed_at_info_level() {
    let (log, buf) = silent_logger(Level::Info);
    log.debug("should not appear").emit();
    assert_eq!(output(&buf), "");
}

#[test]
fn trace_suppressed_at_debug_level() {
    let (log, buf) = silent_logger(Level::Debug);
    log.trace("should not appear").emit();
    assert_eq!(output(&buf), "");
}

#[test]
fn error_passes_at_all_levels() {
    for level in &[Level::Error, Level::Warn, Level::Info, Level::Debug, Level::Trace] {
        let (log, buf) = silent_logger(*level);
        log.error("critical").emit();
        assert!(output(&buf).contains("error(test): critical"), "at level {level:?}: {}", output(&buf));
    }
}

#[test]
fn warn_suppressed_below_warn() {
    let (log, buf) = silent_logger(Level::Error);
    log.warn("should not appear").emit();
    assert_eq!(output(&buf), "");
}

#[test]
fn warn_passes_at_warn_and_below() {
    for level in &[Level::Warn, Level::Info, Level::Debug, Level::Trace] {
        let (log, buf) = silent_logger(*level);
        log.warn("warning").emit();
        assert!(output(&buf).contains("warn(test): warning"), "at level {level:?}");
    }
}

#[test]
fn debug_passes_at_debug_and_below() {
    for level in &[Level::Debug, Level::Trace] {
        let (log, buf) = silent_logger(*level);
        log.debug("debug msg").emit();
        assert!(output(&buf).contains("debug(test): debug msg"), "at level {level:?}");
    }
}

#[test]
fn trace_passes_at_trace() {
    let (log, buf) = silent_logger(Level::Trace);
    log.trace("trace msg").emit();
    assert!(output(&buf).contains("trace(test): trace msg"));
}
