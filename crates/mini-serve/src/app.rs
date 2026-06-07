use std::net::SocketAddr;
use std::sync::Arc;

use hyper::body::Bytes;
use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};
use hyper::service::service_fn;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use hyper_util::server::conn::auto::Builder as AutoBuilder;
use http_body_util::combinators::BoxBody;
use http_body_util::Full;
use tokio::net::TcpListener;

use crate::error::ServeError;
use crate::handler::{Handler, ResponseBody};
use crate::middleware::{CorsConfig, Middleware};
use crate::router::{QueryParams, Router};
use crate::state::State;

pub struct App<S> {
    state:      Arc<S>,
    router:     Arc<Router<S>>,
    cors_config: Option<CorsConfig>,
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

fn error_response(code: u16, message: &str) -> Response<ResponseBody> {
    let body = serde_json::json!({ "message": message });
    let json = serde_json::to_string(&body).unwrap_or_default();
    Response::builder()
        .status(StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
        .header("content-type", "application/json")
        .body(BoxBody::new(Full::new(Bytes::from(json))))
        .unwrap()
}

impl<S: Clone + Send + Sync + 'static> App<S> {
    pub fn new(state: S) -> Self {
        App {
            state:      Arc::new(state),
            router:     Arc::new(Router::new()),
            cors_config: None,
        }
    }

    pub async fn route(&self, req: Request<Incoming>) -> Response<ResponseBody> {
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
                match handler(req, state).await {
                    Ok(resp) => resp,
                    Err(e) => error_response(e.code, &e.message),
                }
            }
            None => {
                if path_exists {
                    error_response(405, "method not allowed")
                } else {
                    error_response(404, "not found")
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
            .map_err(|e| ServeError::new(500, e.to_string()))?;
        let app = Arc::new(self);
        loop {
            let (stream, _) = listener
                .accept()
                .await
                .map_err(|e| ServeError::new(500, e.to_string()))?;
            let app = app.clone();
            tokio::spawn(async move {
                let svc = service_fn(move |req: Request<Incoming>| {
                    let app = app.clone();
                    async move {
                        Ok::<_, hyper::Error>(app.route(req).await)
                    }
                });
                let io = TokioIo::new(stream);
                let _ = AutoBuilder::new(TokioExecutor::new())
                    .serve_connection(io, svc)
                    .await;
            });
        }
    }

    pub async fn bind_ephemeral(self) -> Result<u16, ServeError> {
        let addr: SocketAddr = ([0, 0, 0, 0], 0).into();
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| ServeError::new(500, e.to_string()))?;
        let port = listener
            .local_addr()
            .map_err(|e| ServeError::new(500, e.to_string()))?
            .port();
        let app = Arc::new(self);
        tokio::spawn(async move {
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
                    let _ = AutoBuilder::new(TokioExecutor::new())
                        .serve_connection(io, svc)
                        .await;
                });
            }
        });
        Ok(port)
    }
}

impl App<()> {
    pub fn stateless() -> Self {
        App::new(())
    }
}

#[must_use = "RouteBuilder does nothing until .seal() is called"]
pub struct RouteBuilder<S> {
    state:      Arc<S>,
    router:     Router<S>,
    middleware: Vec<Middleware<S>>,
    cors_config: Option<CorsConfig>,
}

impl<S: Clone + Send + Sync + 'static> RouteBuilder<S> {
    pub fn new(state: S) -> Self {
        RouteBuilder {
            state:       Arc::new(state),
            router:      Router::new(),
            middleware:  Vec::new(),
            cors_config: None,
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
            state:       self.state,
            router:      Arc::new(self.router),
            cors_config: self.cors_config,
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
