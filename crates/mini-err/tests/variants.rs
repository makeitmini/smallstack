use mini_err::{Error, ErrorExt};

// --- 0.1.0 ---

#[test]
fn bad_is_400() {
    let err = Error::bad("api", "invalid input");
    assert_eq!(err.code(), 400);
}

#[test]
fn gone_is_404() {
    let err = Error::gone("db", "record not found");
    assert_eq!(err.code(), 404);
}

#[test]
fn cfg_is_500() {
    let err = Error::cfg("startup", "missing config key");
    assert_eq!(err.code(), 500);
}

#[test]
fn net_is_502() {
    let err = Error::net("upstream", "connection refused");
    assert_eq!(err.code(), 502);
}

#[test]
fn io_is_500() {
    let io = std::io::Error::new(std::io::ErrorKind::Other, "disk full");
    let err = Error::Io { cause: io, scope: "fs" };
    assert_eq!(err.code(), 500);
}

#[test]
fn display_format_is_scope_colon_kind_colon_msg() {
    let err = Error::bad("parse", "missing field 'name'");
    assert_eq!(err.to_string(), "parse:bad: missing field 'name'");
}

#[test]
fn io_conversion_scope_is_io() {
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "no file");
    let err: Error = io.into();
    assert_eq!(err.scope(), "io");
}

#[test]
fn context_preserves_original_message() {
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "config.toml");
    let result: mini_err::Result<i32> = Err(io).context("fs", "failed to open");
    let err = result.unwrap_err();
    assert!(
        err.message().contains("config.toml"),
        "expected original message preserved, got: {}",
        err.message()
    );
}

#[test]
fn context_sets_scope_on_wrapped_error() {
    let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no access");
    let result: mini_err::Result<i32> = Err(io).context("fs", "failed to open");
    let err = result.unwrap_err();
    assert_eq!(err.scope(), "fs");
}

#[test]
fn from_io_error_code_is_500() {
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    let err: Error = io.into();
    assert_eq!(err.code(), 500);
}

// --- 0.2.0 ---

#[test]
fn io_errors_with_same_kind_and_scope_are_equal() {
    let a = Error::Io {
        cause: std::io::Error::new(std::io::ErrorKind::NotFound, "file a"),
        scope: "fs",
    };
    let b = Error::Io {
        cause: std::io::Error::new(std::io::ErrorKind::NotFound, "file b"),
        scope: "fs",
    };
    assert_eq!(a, b);
}

/// --- 0.1.1 ---

#[test]
fn parse_int_error_converts_to_bad() {
    let result: Result<i32, std::num::ParseIntError> = "not_a_number".parse();
    let err: Error = result.unwrap_err().into();
    assert!(matches!(err, Error::Bad { .. }));
    assert_eq!(err.code(), 400);
}

#[test]
fn utf8_error_converts_to_bad() {
    let invalid = &[0xFF, 0xFE, 0x00][..];
    let result = std::str::from_utf8(invalid);
    let err: Error = result.unwrap_err().into();
    assert!(matches!(err, Error::Bad { .. }));
    assert_eq!(err.code(), 400);
}

#[test]
fn from_utf8_error_converts_to_bad() {
    let invalid = vec![0xFF, 0xFE];
    let result = String::from_utf8(invalid);
    let err: Error = result.unwrap_err().into();
    assert!(matches!(err, Error::Bad { .. }));
    assert_eq!(err.code(), 400);
}

// --- Display format for all variants ---

#[test]
fn display_format_for_io_variant() {
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "config.toml");
    let err = Error::Io { cause: io, scope: "fs" };
    assert_eq!(err.to_string(), "fs:io: config.toml");
}

#[test]
fn display_format_for_net_variant() {
    let err = Error::net("upstream", "connection refused");
    assert_eq!(err.to_string(), "upstream:net: connection refused");
}

#[test]
fn display_format_for_cfg_variant() {
    let err = Error::cfg("startup", "missing key");
    assert_eq!(err.to_string(), "startup:cfg: missing key");
}

#[test]
fn display_format_for_gone_variant() {
    let err = Error::gone("db", "record deleted");
    assert_eq!(err.to_string(), "db:gone: record deleted");
}

// --- context() overwrites message for non-Io variants ---

#[test]
fn context_on_bad_overwrites_message() {
    let result: mini_err::Result<i32> = Err(Error::bad("api", "original"))
        .context("new_scope", "new message");
    let err = result.unwrap_err();
    assert_eq!(err.scope(), "new_scope");
    assert_eq!(err.message(), "new message");
    assert_eq!(err.code(), 400);
}

#[test]
fn context_on_gone_overwrites_message() {
    let result: mini_err::Result<i32> = Err(Error::gone("db", "original"))
        .context("api", "new message");
    let err = result.unwrap_err();
    assert_eq!(err.scope(), "api");
    assert_eq!(err.message(), "new message");
    assert_eq!(err.code(), 404);
}

#[test]
fn context_on_net_overwrites_message() {
    let result: mini_err::Result<i32> = Err(Error::net("upstream", "original"))
        .context("proxy", "new message");
    let err = result.unwrap_err();
    assert_eq!(err.scope(), "proxy");
    assert_eq!(err.message(), "new message");
    assert_eq!(err.code(), 502);
}

#[test]
fn context_on_cfg_overwrites_message() {
    let result: mini_err::Result<i32> = Err(Error::cfg("startup", "original"))
        .context("runtime", "new message");
    let err = result.unwrap_err();
    assert_eq!(err.scope(), "runtime");
    assert_eq!(err.message(), "new message");
    assert_eq!(err.code(), 500);
}

// --- source() returns inner error for Io ---

#[test]
fn error_source_for_io_returns_cause() {
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    let err = Error::Io { cause: io, scope: "fs" };
    let source = std::error::Error::source(&err as &dyn std::error::Error);
    assert!(source.is_some(), "Io variant should expose source");
    assert_eq!(source.unwrap().to_string(), "missing");
}

// --- kind() for all variants ---

#[test]
fn kind_strings_are_correct() {
    let io = std::io::Error::new(std::io::ErrorKind::Other, "");
    assert_eq!(Error::Io { cause: io, scope: "" }.kind(), "io");
    assert_eq!(Error::net("", "").kind(), "net");
    assert_eq!(Error::cfg("", "").kind(), "cfg");
    assert_eq!(Error::bad("", "").kind(), "bad");
    assert_eq!(Error::gone("", "").kind(), "gone");
}

// --- cross-variant inequality ---

#[test]
fn different_variants_are_not_equal() {
    assert_ne!(Error::bad("x", "msg"), Error::gone("x", "msg"));
    assert_ne!(Error::cfg("x", "msg"), Error::net("x", "msg"));
}
