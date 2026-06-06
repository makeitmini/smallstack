use std::fmt;

#[derive(Debug)]
pub struct ServeError {
    pub code:    u16,
    pub message: String,
}

impl ServeError {
    pub fn new(code: u16, message: impl Into<String>) -> Self {
        ServeError { code, message: message.into() }
    }
}

impl fmt::Display for ServeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ServeError {}
