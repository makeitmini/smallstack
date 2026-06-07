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
