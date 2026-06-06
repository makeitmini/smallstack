use mini_err::{Error, ErrorExt};

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
