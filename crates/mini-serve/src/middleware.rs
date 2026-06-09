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
    allow_origins:     Vec<String>,
    allow_all_origins: bool,
    allow_methods:     Vec<Method>,
    allow_headers:     Vec<String>,
    expose_headers:    Vec<String>,
    max_age_secs:      Option<usize>,
    credentials:       bool,
}

impl Default for CorsConfig {
    fn default() -> Self {
        CorsConfig {
            allow_origins:     Vec::new(),
            allow_all_origins: false,
            allow_methods:     Vec::new(),
            allow_headers:     Vec::new(),
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

        let origin = if self.credentials && self.allow_all_origins {
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

        if origin != "*" {
            headers.push(("vary".to_string(), "origin".to_string()));
        }

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

        if !self.allow_methods.is_empty() {
            headers.push((
                "access-control-allow-methods".to_string(),
                self.allow_methods
                    .iter()
                    .map(|m| m.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }

        if self.allow_headers.contains(&"*".to_string()) {
            headers.push((
                "access-control-allow-headers".to_string(),
                req_headers.unwrap_or("*").to_string(),
            ));
        } else if !self.allow_headers.is_empty() {
            headers.push((
                "access-control-allow-headers".to_string(),
                self.allow_headers.join(", "),
            ));
        }

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
        let allow_all_origins = self.allow_origins.len() == 1 && self.allow_origins[0] == "*";

        // Credentialed wildcard is a browser-rejected misconfiguration: any
        // origin receives credentials. Panic in debug; warn in release so the
        // mistake surfaces at startup, not silently in production.
        if self.credentials && allow_all_origins {
            #[cfg(debug_assertions)]
            panic!(
                "CORS misconfiguration: allow_origin(\"*\") with allow_credentials(true) \
                 grants credentialed access to every origin. \
                 Use an explicit origin list instead."
            );
            #[cfg(not(debug_assertions))]
            eprintln!(
                "WARNING [mini-serve]: CORS misconfiguration — \
                 wildcard origin with credentials enabled. \
                 This grants credentialed cross-origin access to all origins."
            );
        }

        CorsConfig {
            allow_origins:     self.allow_origins,
            allow_all_origins,
            allow_methods:     self.allow_methods,
            allow_headers:     self.allow_headers,
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
                if n == hyper::header::VARY {
                    if let Some(existing) = resp.headers_mut().get(&n) {
                        if let Ok(existing_str) = existing.to_str() {
                            let merged = format!("{existing_str}, {value}");
                            if let Ok(merged_v) = merged.parse::<hyper::header::HeaderValue>() {
                                resp.headers_mut().insert(n, merged_v);
                                continue;
                            }
                        }
                    }
                }
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
