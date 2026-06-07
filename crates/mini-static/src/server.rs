use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response, StatusCode};
use hyper::service::service_fn;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use hyper_util::server::conn::auto::Builder as AutoBuilder;
use http_body_util::combinators::BoxBody;
use http_body_util::Full;
use tokio::net::TcpListener;

use crate::error::StaticError;
use crate::handler::{Handler, RequestInfo, ResponseBody};
use crate::mime::mime_type;
use crate::resolve::resolve;
use crate::transform::Transform;

pub struct Server {
    dir:       Arc<PathBuf>,
    addr:      SocketAddr,
    handlers:  Vec<Arc<dyn Handler>>,
    transform: Option<Arc<dyn Transform>>,
    #[cfg(feature = "log")]
    logger:    Option<Arc<mini_log::Logger>>,
}

impl Server {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Server {
            dir:       Arc::new(dir.into()),
            addr:      ([127, 0, 0, 1], 0).into(),
            handlers:  Vec::new(),
            transform: None,
            #[cfg(feature = "log")]
            logger:    None,
        }
    }

    pub fn bind(mut self, addr: impl Into<SocketAddr>) -> Self {
        self.addr = addr.into();
        self
    }

    pub fn with_handler(mut self, handler: impl Handler) -> Self {
        self.handlers.push(Arc::new(handler));
        self
    }

    pub fn with_transform(mut self, transform: impl Transform) -> Self {
        self.transform = Some(Arc::new(transform));
        self
    }

    #[cfg(feature = "log")]
    pub fn with_logger(mut self, logger: mini_log::Logger) -> Self {
        self.logger = Some(Arc::new(logger));
        self
    }

    pub async fn run(self) -> Result<(), StaticError> {
        let listener = TcpListener::bind(self.addr)
            .await
            .map_err(StaticError::Io)?;
        let dir = self.dir.canonicalize().map_err(StaticError::Io)?;
        let dir = Arc::new(dir);
        let handlers = Arc::new(self.handlers);
        let transform: Option<Arc<dyn Transform>> = self.transform;
        #[cfg(feature = "log")]
        let logger = self.logger;
        loop {
            let (stream, _) = listener.accept().await.map_err(StaticError::Io)?;
            let dir = dir.clone();
            let handlers = handlers.clone();
            let transform = transform.clone();
            #[cfg(feature = "log")]
            let logger = logger.clone();
            tokio::spawn(async move {
                #[cfg(feature = "log")]
                let logger = logger.clone();
                let svc = service_fn(move |req: Request<Incoming>| {
                    let dir = dir.clone();
                    let handlers = handlers.clone();
                    let transform = transform.clone();
                    #[cfg(feature = "log")]
                    let logger = logger.clone();
                    async move {
                        #![allow(unused_variables)]
                        let method = req.method().to_string();
                        let path = req.uri().path().to_string();
                        let resp = handle(req, &dir, &handlers, &transform).await;
                        let status = resp.status().as_u16();
                        #[cfg(feature = "log")]
                        if let Some(ref l) = logger {
                            l.info("serve")
                                .field("method", &method)
                                .field("path", &path)
                                .field("status", status)
                                .emit();
                        }
                        Ok::<_, Infallible>(resp)
                    }
                });
                let io = TokioIo::new(stream);
                let _ = AutoBuilder::new(TokioExecutor::new())
                    .serve_connection(io, svc)
                    .await;
            });
        }
    }

    pub async fn run_ephemeral(self) -> Result<u16, StaticError> {
        let addr: SocketAddr = ([0, 0, 0, 0], 0).into();
        let listener = TcpListener::bind(addr)
            .await
            .map_err(StaticError::Io)?;
        let port = listener
            .local_addr()
            .map_err(StaticError::Io)?
            .port();
        let dir = self.dir.canonicalize().map_err(StaticError::Io)?;
        let dir = Arc::new(dir);
        let handlers = Arc::new(self.handlers);
        let transform: Option<Arc<dyn Transform>> = self.transform;
        #[cfg(feature = "log")]
        let logger = self.logger;
        tokio::spawn(async move {
            loop {
                let (stream, _) = match listener.accept().await {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let dir = dir.clone();
                let handlers = handlers.clone();
                let transform = transform.clone();
                #[cfg(feature = "log")]
                let logger = logger.clone();
                tokio::spawn(async move {
                    #[cfg(feature = "log")]
                    let logger = logger.clone();
                    let svc = service_fn(move |req: Request<Incoming>| {
                        let dir = dir.clone();
                        let handlers = handlers.clone();
                        let transform = transform.clone();
                        #[cfg(feature = "log")]
                        let logger = logger.clone();
                        async move {
                            #![allow(unused_variables)]
                            let method = req.method().to_string();
                            let path = req.uri().path().to_string();
                            let resp = handle(req, &dir, &handlers, &transform).await;
                            let status = resp.status().as_u16();
                            #[cfg(feature = "log")]
                            if let Some(ref l) = logger {
                                l.info("serve")
                                    .field("method", &method)
                                    .field("path", &path)
                                    .field("status", status)
                                    .emit();
                            }
                            Ok::<_, Infallible>(resp)
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

async fn handle(
    req: Request<Incoming>,
    dir: &Path,
    handlers: &[Arc<dyn Handler>],
    transform: &Option<Arc<dyn Transform>>,
) -> Response<ResponseBody> {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    for handler in handlers {
        let info = RequestInfo {
            method: method.clone(),
            path: path.clone(),
        };
        if let Some(resp) = handler.handle(info).await {
            return resp;
        }
    }

    match resolve(dir, &path) {
        Ok(file_path) => {
            match tokio::fs::read(&file_path).await {
                Ok(mut bytes) => {
                    let mime = mime_type(&file_path);
                    if let Some(ref t) = transform {
                        bytes = t.apply(mime, bytes);
                    }
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", mime)
                        .body(BoxBody::new(Full::new(Bytes::from(bytes))))
                        .unwrap()
                }
                Err(e) => error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("failed to read file: {e}"),
                ),
            }
        }
        Err(e) => {
            let status = e.status_code();
            let msg = e.to_string();
            error_response(status, &msg)
        }
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
