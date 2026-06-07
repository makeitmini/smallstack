use std::path::{Path, PathBuf};

use crate::error::StaticError;

pub fn resolve(root: &Path, request_path: &str) -> Result<PathBuf, StaticError> {
    if request_path.contains('\0') {
        return Err(StaticError::Traversal(request_path.to_string()));
    }

    let decoded = percent_decode(request_path);

    if decoded.contains("..") {
        return Err(StaticError::Traversal(request_path.to_string()));
    }

    let root_canon = root
        .canonicalize()
        .map_err(StaticError::Io)?;

    let stripped = decoded.trim_start_matches('/');
    let joined = root_canon.join(stripped);

    let canon = joined.canonicalize().map_err(|_| {
        StaticError::NotFound(request_path.to_string())
    })?;

    if !canon.starts_with(&root_canon) {
        return Err(StaticError::Traversal(request_path.to_string()));
    }

    if canon.is_dir() {
        let index = canon.join("index.html");
        if index.exists() {
            Ok(index)
        } else {
            Err(StaticError::NotFound(request_path.to_string()))
        }
    } else {
        Ok(canon)
    }
}

fn percent_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                out.push(byte as char);
            } else {
                out.push('%');
                out.push_str(&hex);
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup() -> (tempfile::TempDir, tempfile::TempDir) {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("index.html"), b"<h1>hello</h1>").unwrap();
        fs::create_dir(root.path().join("subdir")).unwrap();
        fs::write(root.path().join("subdir/file.txt"), b"content").unwrap();
        fs::create_dir(root.path().join("empty_dir")).unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("target.txt"), b"outside").unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
        }
        (root, outside)
    }

    #[test]
    fn normal_path_resolves_within_root() {
        let (dir, _outside) = setup();
        let result = resolve(dir.path(), "/index.html").unwrap();
        assert!(result.exists());
        assert_eq!(fs::read_to_string(&result).unwrap(), "<h1>hello</h1>");
    }

    #[test]
    fn nested_path_resolves_correctly() {
        let (dir, _outside) = setup();
        let result = resolve(dir.path(), "/subdir/file.txt").unwrap();
        assert!(result.exists());
        assert_eq!(fs::read_to_string(&result).unwrap(), "content");
    }

    #[test]
    fn root_path_resolves_to_index_html() {
        let (dir, _outside) = setup();
        let result = resolve(dir.path(), "/").unwrap();
        assert_eq!(result.file_name().unwrap(), "index.html");
    }

    #[test]
    fn path_with_dotdot_is_rejected() {
        let (dir, _outside) = setup();
        match resolve(dir.path(), "/../outside.txt") {
            Err(StaticError::Traversal(_)) => {}
            other => panic!("expected Traversal, got {other:?}"),
        }
    }

    #[test]
    fn path_with_encoded_dotdot_is_rejected() {
        let (dir, _outside) = setup();
        match resolve(dir.path(), "/%2e%2e/outside.txt") {
            Err(StaticError::Traversal(_)) => {}
            other => panic!("expected Traversal, got {other:?}"),
        }
    }

    #[test]
    fn path_with_null_byte_is_rejected() {
        let (dir, _outside) = setup();
        match resolve(dir.path(), "/index.html\0") {
            Err(StaticError::Traversal(_)) => {}
            other => panic!("expected Traversal, got {other:?}"),
        }
    }

    #[test]
    fn directory_without_index_returns_not_found() {
        let (dir, _outside) = setup();
        match resolve(dir.path(), "/empty_dir") {
            Err(StaticError::NotFound(_)) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn missing_file_returns_not_found() {
        let (dir, _outside) = setup();
        match resolve(dir.path(), "/nonexistent.html") {
            Err(StaticError::NotFound(_)) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    #[cfg(unix)]
    fn path_outside_root_via_symlink_component_is_rejected() {
        let (dir, _outside) = setup();
        // Request a path that resolves through the symlink to the outside file
        match resolve(dir.path(), "/link/target.txt") {
            Err(StaticError::Traversal(_)) => {}
            other => panic!("expected Traversal, got {other:?}"),
        }
    }
}
