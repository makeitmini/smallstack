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

#[test]
fn path_with_null_byte_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    match resolve(dir.path(), "/index.html\0") {
        Err(StaticError::Traversal(_)) => {}
        other => panic!("expected Traversal, got {other:?}"),
    }
}

#[test]
#[cfg(unix)]
fn path_outside_root_via_symlink_component_is_rejected() {
    use std::fs;
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("target.txt"), b"outside").unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
    match resolve(dir.path(), "/link/target.txt") {
        Err(StaticError::Traversal(_)) => {}
        other => panic!("expected Traversal, got {other:?}"),
    }
}
