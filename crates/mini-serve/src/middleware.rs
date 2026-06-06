use std::sync::Arc;

use hyper::{Method, Response, StatusCode};
use http_body_util::combinators::BoxBody;
use http_body_util::Full;
use hyper::body::Bytes;

use crate::handler::{Handler, ResponseBody};

pub type Middleware<S> =
    Arc<dyn Fn(Handler<S>) -> Handler<S> + Send + Sync>;

pub fn middleware<S, F>(f: F) -> Middleware<S>
where
    S: Clone + Send + Sync + 'static,
    F: Fn(Handler<S>) -> Handler<S> + Send + Sync + 'static,
{
    Arc::new(f)
}

#[derive(Clone)]
pub struct CorsConfig {
    allow_origins:  Vec<String>,
    allow_methods:  Vec<Method>,
    allow_headers:  Vec<String>,
    expose_headers: Vec<String>,
    max_age_secs:   Option<usize>,
    credentials:    bool,
}

impl Default for CorsConfig {
    fn default() -> Self {
        CorsConfig {
            allow_origins:  vec!["*".to_string()],
            allow_methods:  vec![
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::PATCH,
                Method::HEAD,
                Method::OPTIONS,
            ],
            allow_headers:  vec!["*".to_string()],
            expose_headers: Vec::new(),
            max_age_secs:   None,
            credentials:    false,
        }
    }
}

impl CorsConfig {
    pub fn builder() -> CorsConfigBuilder {
        CorsConfigBuilder::default()
    }

    fn build_cors_headers(&self, req_origin: Option<&str>) -> Vec<(String, String)> {
        let mut headers = Vec::new();

        let origin = if self.credentials && self.allow_origins == ["*"] {
            req_origin.unwrap_or("*")
        } else if self.allow_origins.contains(&"*".to_string()) {
            "*"
        } else if let Some(origin) = req_origin {
            if self.allow_origins.iter().any(|o| o == origin) {
                origin
            } else {
                return headers;
            }
        } else {
            return headers;
        };

        headers.push(("access-control-allow-origin".to_string(), origin.to_string()));

        if self.credentials {
            headers.push(("access-control-allow-credentials".to_string(), "true".to_string()));
        }

        if !self.expose_headers.is_empty() {
            headers.push((
                "access-control-expose-headers".to_string(),
                self.expose_headers.join(", "),
            ));
        }

        headers
    }

    fn build_preflight_headers(&self, req_headers: Option<&str>) -> Vec<(String, String)> {
        let mut headers = Vec::new();

        headers.push((
            "access-control-allow-methods".to_string(),
            self.allow_methods
                .iter()
                .map(|m| m.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        ));

        let allowed = if self.allow_headers.contains(&"*".to_string()) {
            req_headers.unwrap_or("*")
        } else {
            &self.allow_headers.join(", ")
        };
        headers.push((
            "access-control-allow-headers".to_string(),
            allowed.to_string(),
        ));

        if let Some(secs) = self.max_age_secs {
            headers.push((
                "access-control-max-age".to_string(),
                secs.to_string(),
            ));
        }

        headers
    }
}

#[derive(Default)]
pub struct CorsConfigBuilder {
    allow_origins:  Vec<String>,
    allow_methods:  Vec<Method>,
    allow_headers:  Vec<String>,
    expose_headers: Vec<String>,
    max_age_secs:   Option<usize>,
    credentials:    bool,
}

impl CorsConfigBuilder {
    pub fn allow_origin(mut self, origin: &str) -> Self {
        self.allow_origins.push(origin.to_string());
        self
    }

    pub fn allow_method(mut self, method: Method) -> Self {
        self.allow_methods.push(method);
        self
    }

    pub fn allow_header(mut self, header: &str) -> Self {
        self.allow_headers.push(header.to_string());
        self
    }

    pub fn expose_header(mut self, header: &str) -> Self {
        self.expose_headers.push(header.to_string());
        self
    }

    pub fn max_age_secs(mut self, secs: usize) -> Self {
        self.max_age_secs = Some(secs);
        self
    }

    pub fn allow_credentials(mut self, yes: bool) -> Self {
        self.credentials = yes;
        self
    }

    pub fn build(self) -> CorsConfig {
        CorsConfig {
            allow_origins:  if self.allow_origins.is_empty() {
                vec!["*".to_string()]
            } else {
                self.allow_origins
            },
            allow_methods:  if self.allow_methods.is_empty() {
                vec![
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::DELETE,
                    Method::PATCH,
                    Method::HEAD,
                ]
            } else {
                self.allow_methods
            },
            allow_headers:  if self.allow_headers.is_empty() {
                vec!["*".to_string()]
            } else {
                self.allow_headers
            },
            expose_headers: self.expose_headers,
            max_age_secs:   self.max_age_secs,
            credentials:    self.credentials,
        }
    }
}

pub(crate) fn apply_headers(
    resp: &mut Response<ResponseBody>,
    headers: Vec<(String, String)>,
) {
    for (name, value) in headers {
        if let Ok(n) = name.parse::<hyper::header::HeaderName>() {
            if let Ok(v) = value.parse::<hyper::header::HeaderValue>() {
                resp.headers_mut().insert(n, v);
            }
        }
    }
}

pub(crate) fn empty_response_with_headers(
    status: StatusCode,
    headers: Vec<(String, String)>,
) -> Response<ResponseBody> {
    let mut resp = Response::builder()
        .status(status)
        .body(BoxBody::new(Full::new(Bytes::new())))
        .unwrap();
    apply_headers(&mut resp, headers);
    resp
}

impl CorsConfig {
    pub fn preflight_response(
        &self,
        req_origin: Option<&str>,
        req_acrh: Option<&str>,
    ) -> Response<ResponseBody> {
        let mut headers = self.build_cors_headers(req_origin);
        headers.extend(self.build_preflight_headers(req_acrh));
        empty_response_with_headers(StatusCode::NO_CONTENT, headers)
    }

    pub fn apply_to_response(
        &self,
        resp: &mut Response<ResponseBody>,
        req_origin: Option<&str>,
    ) {
        let headers = self.build_cors_headers(req_origin);
        apply_headers(resp, headers);
    }
}

// CORS is handled at the App level via .with_cors() on RouteBuilder.
// A handler-wrapper middleware cannot intercept preflight before routing,
// so we do not expose a cors() middleware function here.
