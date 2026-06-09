use std::io::Write;
use std::sync::{Arc, Mutex};

use crate::entry::Entry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Conventional,
    Json,
}

pub struct Logger {
    pub level:  Level,
    pub format: Format,
    pub scope:  &'static str,
    pub out:    Arc<Mutex<Box<dyn Write + Send + Sync>>>,
}

impl Clone for Logger {
    fn clone(&self) -> Self {
        Logger {
            level:  self.level,
            format: self.format,
            scope:  self.scope,
            out:    self.out.clone(),
        }
    }
}

impl Logger {
    pub fn new(scope: &'static str) -> Self {
        Logger {
            level:  Level::Info,
            format: Format::Conventional,
            scope,
            out:    Arc::new(Mutex::new(Box::new(std::io::stdout()))),
        }
    }

    pub fn with_level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    pub fn with_format(mut self, format: Format) -> Self {
        self.format = format;
        self
    }

    pub fn with_writer(mut self, out: Arc<Mutex<Box<dyn Write + Send + Sync>>>) -> Self {
        self.out = out;
        self
    }

    /// Build a logger from environment variables.
    ///
    /// - `LOG_LEVEL`: `error`, `warn`, `info`, `debug`, `trace`.
    ///   Unset or unrecognized → `Level::Info`.
    /// - `LOG_FORMAT`: `conventional` or `json`.
    ///   Unset → `Format::Conventional`.
    pub fn from_env(scope: &'static str) -> Self {
        let level = std::env::var("LOG_LEVEL")
            .ok()
            .and_then(|s| match s.to_ascii_lowercase().as_str() {
                "error" => Some(Level::Error),
                "warn"  => Some(Level::Warn),
                "info"  => Some(Level::Info),
                "debug" => Some(Level::Debug),
                "trace" => Some(Level::Trace),
                _       => None,
            })
            .unwrap_or(Level::Info);

        let format = std::env::var("LOG_FORMAT")
            .ok()
            .and_then(|s| match s.to_ascii_lowercase().as_str() {
                "conventional" => Some(Format::Conventional),
                "json"         => Some(Format::Json),
                _              => None,
            })
            .unwrap_or(Format::Conventional);

        Logger {
            level,
            format,
            scope,
            out: Arc::new(Mutex::new(Box::new(std::io::stdout()))),
        }
    }

    pub fn error(&self, msg: &'static str) -> Entry<'_> {
        Entry { logger: self, level: Level::Error, msg, fields: Default::default(), count: 0, overflow_count: 0 }
    }

    pub fn warn(&self, msg: &'static str) -> Entry<'_> {
        Entry { logger: self, level: Level::Warn, msg, fields: Default::default(), count: 0, overflow_count: 0 }
    }

    pub fn info(&self, msg: &'static str) -> Entry<'_> {
        Entry { logger: self, level: Level::Info, msg, fields: Default::default(), count: 0, overflow_count: 0 }
    }

    pub fn debug(&self, msg: &'static str) -> Entry<'_> {
        Entry { logger: self, level: Level::Debug, msg, fields: Default::default(), count: 0, overflow_count: 0 }
    }

    pub fn trace(&self, msg: &'static str) -> Entry<'_> {
        Entry { logger: self, level: Level::Trace, msg, fields: Default::default(), count: 0, overflow_count: 0 }
    }
}
