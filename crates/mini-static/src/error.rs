use std::convert::Infallible;
use std::fmt;

use hyper::StatusCode;

#[derive(Debug)]
#[non_exhaustive]
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
    /// User-safe message that does not leak internal paths or OS error details.
    pub fn user_message(&self) -> String {
        match self {
            StaticError::NotFound(_) => "not found".to_string(),
            StaticError::Traversal(_) => "path traversal denied".to_string(),
            StaticError::Io(_) => "internal server error".to_string(),
        }
    }

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

impl From<Infallible> for StaticError {
    fn from(e: Infallible) -> Self {
        match e {}
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
                msg: None,
            },
        }
    }
}
