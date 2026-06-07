use mini_log::{Format, Level, Logger};

#[test]
fn from_env_fallbacks() {
    std::env::remove_var("LOG_LEVEL");
    std::env::remove_var("LOG_FORMAT");
    let log = Logger::from_env("test");
    assert_eq!(
        log.level,
        Level::Info,
        "expected Info when LOG_LEVEL is unset"
    );
    assert_eq!(
        log.format,
        Format::Conventional,
        "expected Conventional when LOG_FORMAT is unset"
    );

    std::env::set_var("LOG_LEVEL", "bogus");
    let log = Logger::from_env("test");
    assert_eq!(
        log.level,
        Level::Info,
        "expected Info when LOG_LEVEL is unknown"
    );

    std::env::set_var("LOG_LEVEL", "debug");
    std::env::set_var("LOG_FORMAT", "json");
    let log = Logger::from_env("test");
    assert_eq!(log.level, Level::Debug, "expected Debug when LOG_LEVEL=debug");
    assert_eq!(
        log.format,
        Format::Json,
        "expected Json when LOG_FORMAT=json"
    );
}
