use std::fs;

use mini_static::resolve;
use mini_static::StaticError;

#[test]
fn dotdot_path_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("index.html"), b"ok").unwrap();
    match resolve(dir.path(), "/../etc/passwd") {
        Err(StaticError::Traversal(_)) => {}
        other => panic!("expected Traversal, got {other:?}"),
    }
}

#[test]
fn encoded_dotdot_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("index.html"), b"ok").unwrap();
    match resolve(dir.path(), "/%2e%2e/etc") {
        Err(StaticError::Traversal(_)) => {}
        other => panic!("expected Traversal, got {other:?}"),
    }
}

#[test]
fn missing_file_returns_not_found() {
    let dir = tempfile::tempdir().unwrap();
    match resolve(dir.path(), "/nonexistent.txt") {
        Err(StaticError::NotFound(_)) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}
