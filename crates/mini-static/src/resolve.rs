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

/// Decode percent-encoded sequences in a URL path.
///
/// # ASCII-only contract
///
/// This decoder treats each `%XX` pair as a single byte and casts it to
/// `char` via `byte as char`. This is correct for ASCII (0x00–0x7F) and
/// intentionally produces replacement characters for multi-byte UTF-8
/// sequences such as `%C3%A9` (`é`). Non-ASCII paths will fail
/// `Path::canonicalize` and return `StaticError::NotFound`.
///
/// **Do not change this to a UTF-8-aware decoder.** The traversal guard
/// is `canonicalize()` + `starts_with(root)`, which is the authoritative
/// check. The `..` substring check is an early-exit optimisation only.
/// A UTF-8-aware decoder that reassembles multi-byte sequences would need
/// its own traversal analysis; this function deliberately avoids that
/// complexity.
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
