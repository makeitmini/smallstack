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
    assert_eq!(output(&buf), "info(test): started\n");
}

#[test]
fn field_appears_in_output_as_key_eq_value() {
    let (log, buf) = test_logger();
    log.info("request").field("method", "GET").emit();
    assert_eq!(output(&buf), "info(test): request method=GET\n");
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
    assert_eq!(output(&buf), "info(test): many_fields a=1 b=2 c=3 d=4 e=5 f=6 g=7 h=8\n");
}
