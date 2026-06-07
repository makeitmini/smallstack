use std::fmt;

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Io  { cause: std::io::Error, scope: &'static str },
    Net { msg: String,           scope: &'static str },
    Cfg { msg: String,           scope: &'static str },
    Bad { msg: String,           scope: &'static str },
    Gone { msg: String,          scope: &'static str },
}

impl Error {
    pub fn bad(scope: &'static str, msg: impl Into<String>) -> Self {
        Error::Bad { msg: msg.into(), scope }
    }

    pub fn gone(scope: &'static str, msg: impl Into<String>) -> Self {
        Error::Gone { msg: msg.into(), scope }
    }

    pub fn cfg(scope: &'static str, msg: impl Into<String>) -> Self {
        Error::Cfg { msg: msg.into(), scope }
    }

    pub fn net(scope: &'static str, msg: impl Into<String>) -> Self {
        Error::Net { msg: msg.into(), scope }
    }

    pub fn code(&self) -> u16 {
        match self {
            Error::Io { .. }  => 500,
            Error::Net { .. } => 502,
            Error::Cfg { .. } => 500,
            Error::Bad { .. } => 400,
            Error::Gone { .. } => 404,
        }
    }

    pub fn scope(&self) -> &'static str {
        match self {
            Error::Io { scope, .. }  => scope,
            Error::Net { scope, .. } => scope,
            Error::Cfg { scope, .. } => scope,
            Error::Bad { scope, .. } => scope,
            Error::Gone { scope, .. } => scope,
        }
    }

    pub fn message(&self) -> String {
        match self {
            Error::Io { cause, .. } => cause.to_string(),
            Error::Net { msg, .. } => msg.clone(),
            Error::Cfg { msg, .. }  => msg.clone(),
            Error::Bad { msg, .. }  => msg.clone(),
            Error::Gone { msg, .. } => msg.clone(),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Error::Io { .. }  => "io",
            Error::Net { .. } => "net",
            Error::Cfg { .. } => "cfg",
            Error::Bad { .. } => "bad",
            Error::Gone { .. } => "gone",
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(cause: std::io::Error) -> Self {
        Error::Io { cause, scope: "io" }
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(e: std::num::ParseIntError) -> Self {
        Error::Bad { msg: e.to_string(), scope: "parse" }
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(e: std::str::Utf8Error) -> Self {
        Error::Bad { msg: e.to_string(), scope: "utf8" }
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(e: std::string::FromUtf8Error) -> Self {
        Error::Bad { msg: e.utf8_error().to_string(), scope: "utf8" }
    }
}

/// Display format: `{scope}:{kind}: {msg}`  e.g. `parse:bad: missing field 'name'`
///
/// This format is stable. Downstream code may rely on it for parsing.
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Error::Io { .. }  => "io",
            Error::Net { .. } => "net",
            Error::Cfg { .. } => "cfg",
            Error::Bad { .. } => "bad",
            Error::Gone { .. } => "gone",
        };
        write!(f, "{}:{}: {}", self.scope(), kind, self.message())
    }
}

impl std::error::Error for Error {}

/// Equality for `Error`.
///
/// For `Io`, compares by `std::io::ErrorKind` and `scope` only —
/// the inner `std::io::Error` does not implement `PartialEq`.
/// For all other variants, compares by `msg` and `scope`.
impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Error::Io { cause: a, scope: sa }, Error::Io { cause: b, scope: sb }) => {
                a.kind() == b.kind() && sa == sb
            }
            (Error::Net { msg: a, scope: sa }, Error::Net { msg: b, scope: sb }) => {
                a == b && sa == sb
            }
            (Error::Cfg { msg: a, scope: sa }, Error::Cfg { msg: b, scope: sb }) => {
                a == b && sa == sb
            }
            (Error::Bad { msg: a, scope: sa }, Error::Bad { msg: b, scope: sb }) => {
                a == b && sa == sb
            }
            (Error::Gone { msg: a, scope: sa }, Error::Gone { msg: b, scope: sb }) => {
                a == b && sa == sb
            }
            _ => false,
        }
    }
}
