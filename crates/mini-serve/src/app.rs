use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use hyper::body::Bytes;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::{Method, Request, Response, StatusCode};
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use hyper_util::rt::TokioTimer;
use http_body_util::combinators::BoxBody;
use http_body_util::{Empty, Full};
use tokio::net::TcpListener;

use crate::body::{MaxBodySize, DEFAULT_MAX_BODY_SIZE};
use crate::error::ServeError;
use crate::handler::{Handler, ResponseBody};
use crate::middleware::{CorsConfig, Middleware};
use crate::router::{QueryParams, Router};
use crate::state::State;

/// Maximum request URI path length in bytes.
const MAX_PATH_LEN: usize = 8_192;

/// Maximum query string length in bytes.
const MAX_QUERY_LEN: usize = 4_096;

/// Default timeout for reading request headers (idle timeout per-request).
const DEFAULT_HEADER_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Application-level error handler. Replaces the default JSON error responses.
pub type ErrorHandler =
    Arc<dyn Fn(StatusCode, &str) -> Response<ResponseBody> + Send + Sync>;

pub struct App<S> {
    state:               Arc<S>,
    router:              Arc<Router<S>>,
    cors_config:         Option<CorsConfig>,
    max_body_size:       usize,
    header_read_timeout: Duration,
    error_handler:       ErrorHandler,
}

fn parse_query(query: Option<&str>) -> QueryParams {
    let mut map = std::collections::HashMap::new();
    if let Some(query) = query {
        for pair in query.split('&').filter(|s| !s.is_empty()) {
            if let Some((key, value)) = pair.split_once('=') {
                map.insert(key.to_string(), value.to_string());
            } else {
                map.insert(pair.to_string(), String::new());
            }
        }
    }
    QueryParams(map)
}

// Inline tests OK per STANDARDS.md: parse_query is a pure-logic
// helper with no public API surface.
#[cfg(test)]
mod parse_query_tests {
    use super::*;

    #[test]
    fn none_returns_empty() {
        let p = parse_query(None);
        assert!(p.0.is_empty());
    }

    #[test]
    fn empty_string_returns_empty() {
        let p = parse_query(Some(""));
        assert!(p.0.is_empty());
    }

    #[test]
    fn single_key_value_pair() {
        let p = parse_query(Some("name=alice"));
        assert_eq!(p.0.get("name").unwrap(), "alice");
    }

    #[test]
    fn multiple_pairs() {
        let p = parse_query(Some("a=1&b=2&c=3"));
        assert_eq!(p.0.get("a").unwrap(), "1");
        assert_eq!(p.0.get("b").unwrap(), "2");
        assert_eq!(p.0.get("c").unwrap(), "3");
    }

    #[test]
    fn key_without_equals_gets_empty_value() {
        let p = parse_query(Some("flag"));
        assert_eq!(p.0.get("flag").unwrap(), "");
    }

    #[test]
    fn empty_value_after_equals() {
        let p = parse_query(Some("key="));
        assert_eq!(p.0.get("key").unwrap(), "");
    }

    #[test]
    fn repeated_key_last_wins() {
        let p = parse_query(Some("key=first&key=second"));
        assert_eq!(p.0.get("key").unwrap(), "second");
    }

    #[test]
    fn percent_encoded_value_is_preserved() {
        let p = parse_query(Some("q=hello%20world"));
        assert_eq!(p.0.get("q").unwrap(), "hello%20world");
    }

    #[test]
    fn pair_with_multiple_equals_uses_first_split() {
        let p = parse_query(Some("key=a=b=c"));
        assert_eq!(p.0.get("key").unwrap(), "a=b=c");
    }
}

fn error_response(status: StatusCode, message: &str) -> Response<ResponseBody> {
    let body = serde_json::json!({ "message": message });
    let json = serde_json::to_string(&body).unwrap_or_default();
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(BoxBody::new(Full::new(Bytes::from(json))))
        .unwrap()
}

fn default_error_handler() -> ErrorHandler {
    Arc::new(|status, message| error_response(status, message))
}

impl<S: Clone + Send + Sync + 'static> App<S> {
    pub fn new(state: S) -> Self {
        App {
            state:              Arc::new(state),
            router:             Arc::new(Router::new()),
            cors_config:        None,
            max_body_size:      DEFAULT_MAX_BODY_SIZE,
            header_read_timeout: DEFAULT_HEADER_READ_TIMEOUT,
            error_handler:      default_error_handler(),
        }
    }

    pub async fn route(&self, req: Request<Incoming>) -> Response<ResponseBody> {
        // Reject oversized path or query before any allocation or routing.
        if req.uri().path().len() > MAX_PATH_LEN {
            return (self.error_handler)(StatusCode::BAD_REQUEST, "path too long");
        }
        if req.uri().query().map(|q| q.len()).unwrap_or(0) > MAX_QUERY_LEN {
            return (self.error_handler)(StatusCode::BAD_REQUEST, "query string too long");
        }

        let method = req.method().clone();
        let path = req.uri().path().to_string();
        let state = State::new(S::clone(&self.state));

        // Read CORS-relevant headers before req is consumed
        let req_origin = req
            .headers()
            .get("origin")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let req_acrh = req
            .headers()
            .get("access-control-request-headers")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // CORS preflight — handled before routing
        if let Some(cfg) = &self.cors_config {
            if method == Method::OPTIONS && req_origin.is_some() {
                return cfg.preflight_response(req_origin.as_deref(), req_acrh.as_deref());
            }
        }

        let path_exists = self.router.has_path(&path);
        let query_params = parse_query(req.uri().query());

        let mut resp = match self.router.match_route(&method, &path) {
            Some((handler, params)) => {
                let mut req = req;
                req.extensions_mut().insert(query_params);
                req.extensions_mut().insert(params);
                req.extensions_mut().insert(MaxBodySize(self.max_body_size));
                match handler(req, state).await {
                    Ok(resp) => resp,
                    Err(e) => (self.error_handler)(
                        StatusCode::from_u16(e.code)
                            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                        &e.message,
                    ),
                }
            }
            None => {
                if method == Method::HEAD {
                    // RFC 7231 §4.3.2: HEAD may be served by a GET handler
                    if self.router.has_path(&path) {
                        match self.router.match_route(&Method::GET, &path) {
                            Some((handler, params)) => {
                                let mut req = req;
                                req.extensions_mut().insert(query_params);
                                req.extensions_mut().insert(params);
                                req.extensions_mut().insert(MaxBodySize(self.max_body_size));
                                match handler(req, state).await {
                                    Ok(resp) => {
                                        // RFC 7231 §4.3.2: HEAD must return identical
                                        // headers to GET but with an empty body.
                                        let (mut parts, _body) = resp.into_parts();
                                        parts.headers.insert(
                                            hyper::header::CONTENT_LENGTH,
                                            hyper::header::HeaderValue::from_static("0"),
                                        );
                                        Response::from_parts(parts, BoxBody::new(Empty::new()))
                                    }
                                    Err(e) => (self.error_handler)(
                                        StatusCode::from_u16(e.code)
                                            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                                        &e.message,
                                    ),
                                }
                            }
                            None => (self.error_handler)(StatusCode::NOT_FOUND, "not found"),
                        }
                    } else {
                        (self.error_handler)(StatusCode::NOT_FOUND, "not found")
                    }
                } else if path_exists {
                    (self.error_handler)(StatusCode::METHOD_NOT_ALLOWED, "method not allowed")
                } else {
                    (self.error_handler)(StatusCode::NOT_FOUND, "not found")
                }
            }
        };

        if let Some(cfg) = &self.cors_config {
            cfg.apply_to_response(&mut resp, req_origin.as_deref());
        }

        resp
    }

    pub async fn bind(self, addr: SocketAddr) -> Result<(), ServeError> {
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|_| ServeError::new(500, format!("failed to bind to {addr}")))?;
        let app = Arc::new(self);
        serve_inner(listener, app).await;
        Ok(())
    }

    pub async fn bind_ephemeral(self) -> Result<u16, ServeError> {
        let addr: SocketAddr = ([0, 0, 0, 0], 0).into();
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|_| ServeError::new(500, "failed to bind to ephemeral port"))?;
        let port = listener
            .local_addr()
            .map_err(|_| ServeError::new(500, "failed to get assigned port"))?
            .port();
        let app = Arc::new(self);
        tokio::spawn(async move {
            serve_inner(listener, app).await;
        });
        Ok(port)
    }
}

impl App<()> {
    pub fn stateless() -> Self {
        App::new(())
    }
}

async fn serve_inner<S: Clone + Send + Sync + 'static>(
    listener: TcpListener,
    app: Arc<App<S>>,
) {
    let header_read_timeout = app.header_read_timeout;
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };
        let app = app.clone();
        tokio::spawn(async move {
            let svc = service_fn(move |req: Request<Incoming>| {
                let app = app.clone();
                async move {
                    Ok::<_, hyper::Error>(app.route(req).await)
                }
            });
            let io = TokioIo::new(stream);
            let mut builder = http1::Builder::new();
            builder.timer(TokioTimer::new());
            builder.header_read_timeout(header_read_timeout);
            let conn = builder.serve_connection(io, svc);
            let _ = conn.await;
        });
    }
}

#[must_use = "RouteBuilder does nothing until .seal() is called"]
pub struct RouteBuilder<S> {
    state:               Arc<S>,
    router:              Router<S>,
    middleware:          Vec<Middleware<S>>,
    cors_config:         Option<CorsConfig>,
    max_body_size:       usize,
    header_read_timeout: Duration,
    error_handler:       ErrorHandler,
}

impl<S: Clone + Send + Sync + 'static> RouteBuilder<S> {
    pub fn new(state: S) -> Self {
        RouteBuilder {
            state:               Arc::new(state),
            router:              Router::new(),
            middleware:          Vec::new(),
            cors_config:         None,
            max_body_size:       DEFAULT_MAX_BODY_SIZE,
            header_read_timeout: DEFAULT_HEADER_READ_TIMEOUT,
            error_handler:       default_error_handler(),
        }
    }

    pub fn wrap(mut self, m: Middleware<S>) -> Self {
        self.middleware.push(m);
        self
    }

    pub fn with_cors(mut self, config: CorsConfig) -> Self {
        self.cors_config = Some(config);
        self
    }

    pub fn with_max_body_size(mut self, max: usize) -> Self {
        self.max_body_size = max;
        self
    }

    pub fn with_header_read_timeout(mut self, d: Duration) -> Self {
        self.header_read_timeout = d;
        self
    }

    pub fn with_error_handler(
        mut self,
        f: impl Fn(StatusCode, &str) -> Response<ResponseBody> + Send + Sync + 'static,
    ) -> Self {
        self.error_handler = Arc::new(f);
        self
    }

    pub fn get(mut self, path: &str, handler: Handler<S>) -> Self {
        self.router.insert(Method::GET, path, handler);
        self
    }

    pub fn post(mut self, path: &str, handler: Handler<S>) -> Self {
        self.router.insert(Method::POST, path, handler);
        self
    }

    pub fn put(mut self, path: &str, handler: Handler<S>) -> Self {
        self.router.insert(Method::PUT, path, handler);
        self
    }

    pub fn delete(mut self, path: &str, handler: Handler<S>) -> Self {
        self.router.insert(Method::DELETE, path, handler);
        self
    }

    pub fn seal(mut self) -> App<S> {
        let middleware = std::mem::take(&mut self.middleware);
        self.router.apply_middleware(&middleware);
        App {
            state:              self.state,
            router:             Arc::new(self.router),
            cors_config:         self.cors_config,
            max_body_size:       self.max_body_size,
            header_read_timeout: self.header_read_timeout,
            error_handler:       self.error_handler,
        }
    }
}

impl<S: Clone + Send + Sync + 'static> RouteBuilder<S> {
    /// Register a group of routes sharing an optional prefix and group-level middleware.
    ///
    /// The closure receives a [`GroupBuilder`] whose routes are prefixed with `prefix`.
    /// Group middleware is applied before parent middleware.
    pub fn group<F>(mut self, prefix: &str, f: F) -> Self
    where
        F: FnOnce(GroupBuilder<S>) -> GroupBuilder<S>,
    {
        let group = f(GroupBuilder::new());
        let mut routes = group.routes;

        for m in &group.middleware {
            for (_, _, handler) in &mut routes {
                *handler = m(handler.clone());
            }
        }

        let prefix = prefix.trim_matches('/');
        for (method, path, handler) in routes {
            let path = path.trim_start_matches('/');
            let full_path = if prefix.is_empty() {
                format!("/{path}")
            } else {
                format!("/{prefix}/{path}")
            };
            self.router.insert(method, &full_path, handler);
        }

        self
    }
}

impl RouteBuilder<()> {
    pub fn stateless() -> Self {
        RouteBuilder::new(())
    }
}

/// Builder for routes inside a [`RouteBuilder::group`].
///
/// Supports the same method-specific routing and middleware as [`RouteBuilder`]
/// but routes are automatically prefixed with the group's path prefix.
#[must_use = "GroupBuilder methods return Self; call .get(), .post(), etc., and return from the closure"]
pub struct GroupBuilder<S> {
    routes:     Vec<(Method, String, Handler<S>)>,
    middleware: Vec<Middleware<S>>,
}

impl<S: Clone + Send + Sync + 'static> GroupBuilder<S> {
    fn new() -> Self {
        GroupBuilder {
            routes:     Vec::new(),
            middleware: Vec::new(),
        }
    }

    pub fn wrap(mut self, m: Middleware<S>) -> Self {
        self.middleware.push(m);
        self
    }

    pub fn get(mut self, path: &str, handler: Handler<S>) -> Self {
        self.routes.push((Method::GET, path.to_string(), handler));
        self
    }

    pub fn post(mut self, path: &str, handler: Handler<S>) -> Self {
        self.routes.push((Method::POST, path.to_string(), handler));
        self
    }

    pub fn put(mut self, path: &str, handler: Handler<S>) -> Self {
        self.routes.push((Method::PUT, path.to_string(), handler));
        self
    }

    pub fn delete(mut self, path: &str, handler: Handler<S>) -> Self {
        self.routes.push((Method::DELETE, path.to_string(), handler));
        self
    }
}
