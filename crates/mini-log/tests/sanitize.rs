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
fn clean_input_is_unchanged() {
    let (log, buf) = test_logger();
    let clean = "method=GET path=/api/users status=200";
    log.info("test")
        .field("key", clean)
        .emit();
    let out = output(&buf);
    assert!(out.contains("key=method=GET path=/api/users status=200"));
}

#[test]
fn newline_is_replaced() {
    let (log, buf) = test_logger();
    log.info("test")
        .field("key", "injected\nheader: evil")
        .emit();
    let out = output(&buf);
    assert!(!out.contains("injected\nheader"), "newline must be replaced");
    assert!(out.contains("injected header: evil"));
}

#[test]
fn carriage_return_is_replaced() {
    let (log, buf) = test_logger();
    log.info("test")
        .field("key", "value\roverflow")
        .emit();
    let out = output(&buf);
    assert!(!out.contains('\r'), "carriage return must be replaced");
    assert!(out.contains("value overflow"));
}

#[test]
fn null_byte_is_replaced() {
    let (log, buf) = test_logger();
    log.info("test")
        .field("key", "value\0truncated")
        .emit();
    let out = output(&buf);
    assert!(!out.contains('\0'), "null byte must be replaced");
    assert!(out.contains("value truncated"));
}

#[test]
fn ansi_escape_is_stripped() {
    let (log, buf) = test_logger();
    log.info("test")
        .field("key", "normal\x1b[31mRED TEXT\x1b[0m end")
        .emit();
    let out = output(&buf);
    assert!(!out.contains('\x1b'), "escape byte must be removed");
    assert!(out.contains("normal"), "surrounding text must be preserved");
    assert!(out.contains("RED TEXT"), "visible text must be preserved");
    assert!(out.contains("end"), "trailing text must be preserved");
}

#[test]
fn tab_is_preserved() {
    let (log, buf) = test_logger();
    log.info("test")
        .field("key", "col1\tcol2")
        .emit();
    let out = output(&buf);
    assert!(out.contains("col1\tcol2"), "tab must be preserved in output");
}

#[test]
fn printable_ascii_is_unchanged() {
    let (log, buf) = test_logger();
    let input = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789 !@#$%";
    log.info("test")
        .field("key", input)
        .emit();
    let out = output(&buf);
    assert!(out.contains(input), "printable ASCII must be unchanged");
}
