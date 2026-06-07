use std::fmt;

use hyper::StatusCode;

#[derive(Debug)]
pub enum StaticError {
    NotFound(String),
    Traversal(String),
    Io(std::io::Error),
}

impl fmt::Display for StaticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StaticError::NotFound(path) => write!(f, "not found: {path}"),
            StaticError::Traversal(path) => write!(f, "path traversal denied: {path}"),
            StaticError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for StaticError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StaticError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl StaticError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            StaticError::NotFound(_) => StatusCode::NOT_FOUND,
            StaticError::Traversal(_) => StatusCode::FORBIDDEN,
            StaticError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<std::io::Error> for StaticError {
    fn from(e: std::io::Error) -> Self {
        StaticError::Io(e)
    }
}

#[cfg(feature = "err")]
impl From<StaticError> for mini_err::Error {
    fn from(e: StaticError) -> Self {
        match e {
            StaticError::NotFound(msg) => {
                mini_err::Error::gone("static", format!("not found: {msg}"))
            }
            StaticError::Traversal(msg) => {
                mini_err::Error::bad("static", format!("path traversal denied: {msg}"))
            }
            StaticError::Io(cause) => mini_err::Error::Io {
                cause,
                scope: "static",
            },
        }
    }
}
