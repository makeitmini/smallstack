use std::fmt;

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    NotFound { kind: &'static str, id: String },
    InvalidQuery { msg: String },
    InvalidValue { msg: String },
    Store { msg: String },
    Io { cause: std::io::Error },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn not_found(kind: &'static str, id: impl Into<String>) -> Self {
        Error::NotFound { kind, id: id.into() }
    }

    pub fn invalid_query(msg: impl Into<String>) -> Self {
        Error::InvalidQuery { msg: msg.into() }
    }

    pub fn invalid_value(msg: impl Into<String>) -> Self {
        Error::InvalidValue { msg: msg.into() }
    }

    pub fn store(msg: impl Into<String>) -> Self {
        Error::Store { msg: msg.into() }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Error::NotFound { .. } => "not_found",
            Error::InvalidQuery { .. } => "invalid_query",
            Error::InvalidValue { .. } => "invalid_value",
            Error::Store { .. } => "store",
            Error::Io { .. } => "io",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound { kind, id } => {
                write!(f, "Not found: {kind} with id {id}")
            }
            Error::InvalidQuery { msg } => write!(f, "Invalid query: {msg}"),
            Error::InvalidValue { msg } => write!(f, "Invalid value: {msg}"),
            Error::Store { msg } => write!(f, "Store error: {msg}"),
            Error::Io { cause } => write!(f, "I/O error: {cause}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { cause } => Some(cause),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(cause: std::io::Error) -> Self {
        Error::Io { cause }
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Error::NotFound { kind: k1, id: i1 },
                Error::NotFound { kind: k2, id: i2 },
            ) => k1 == k2 && i1 == i2,
            (
                Error::InvalidQuery { msg: m1 },
                Error::InvalidQuery { msg: m2 },
            ) => m1 == m2,
            (
                Error::InvalidValue { msg: m1 },
                Error::InvalidValue { msg: m2 },
            ) => m1 == m2,
            (Error::Store { msg: m1 }, Error::Store { msg: m2 }) => m1 == m2,
            (Error::Io { .. }, Error::Io { .. }) => false,
            _ => false,
        }
    }
}
