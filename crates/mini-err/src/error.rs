use std::fmt;

#[derive(Debug)]
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
}

impl From<std::io::Error> for Error {
    fn from(cause: std::io::Error) -> Self {
        Error::Io { cause, scope: "io" }
    }
}

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
