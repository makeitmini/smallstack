use std::sync::Arc;

use crate::handler::handler;
use crate::middleware::middleware;
use crate::middleware::Middleware;

fn level_for_status(code: u16) -> mini_log::Level {
    if code >= 500 {
        mini_log::Level::Error
    } else if code >= 400 {
        mini_log::Level::Warn
    } else {
        mini_log::Level::Info
    }
}

/// Logging middleware that records method, path, status, and duration
/// for every request handled through the wrapped handler.
pub struct LoggingMiddleware {
    logger: Arc<mini_log::Logger>,
}

impl LoggingMiddleware {
    pub fn new(logger: mini_log::Logger) -> Self {
        Self {
            logger: Arc::new(logger),
        }
    }

    pub fn middleware<S: Clone + Send + Sync + 'static>(&self) -> Middleware<S> {
        let logger = self.logger.clone();
        middleware(move |inner_handler| {
            let logger = logger.clone();
            handler(move |req, state| {
                let logger = logger.clone();
                let inner = inner_handler.clone();
                let start = std::time::Instant::now();
                let method = req.method().to_string();
                let path = req.uri().path().to_string();
                async move {
                    let result = inner(req, state).await;
                    match &result {
                        Ok(resp) => {
                            let status = resp.status().as_u16();
                            let lvl = level_for_status(status);
                            let entry = match lvl {
                                mini_log::Level::Error => logger.error("request"),
                                mini_log::Level::Warn => logger.warn("request"),
                                _ => logger.info("request"),
                            };
                            entry
                                .field("method", &method)
                                .field("path", &path)
                                .field("status", status)
                                .duration(start)
                                .emit();
                        }
                        Err(e) => {
                            let lvl = level_for_status(e.code);
                            let entry = match lvl {
                                mini_log::Level::Error => logger.error("request"),
                                mini_log::Level::Warn => logger.warn("request"),
                                _ => logger.info("request"),
                            };
                            entry
                                .field("method", &method)
                                .field("path", &path)
                                .field("status", e.code)
                                .duration(start)
                                .emit();
                        }
                    }
                    result
                }
            })
        })
    }
}
