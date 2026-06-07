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
use crate::mime::mime_type;
use crate::resolve::resolve;

type ResponseBody = BoxBody<Bytes, Infallible>;

pub struct Server {
    dir:  Arc<PathBuf>,
    addr: SocketAddr,
}

impl Server {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Server {
            dir:  Arc::new(dir.into()),
            addr: ([127, 0, 0, 1], 0).into(),
        }
    }

    pub fn bind(mut self, addr: impl Into<SocketAddr>) -> Self {
        self.addr = addr.into();
        self
    }

    pub async fn run(self) -> Result<(), StaticError> {
        let listener = TcpListener::bind(self.addr)
            .await
            .map_err(StaticError::Io)?;
        let dir = self.dir.canonicalize().map_err(StaticError::Io)?;
        let dir = Arc::new(dir);
        loop {
            let (stream, _) = listener.accept().await.map_err(StaticError::Io)?;
            let dir = dir.clone();
            tokio::spawn(async move {
                let svc = service_fn(move |req: Request<Incoming>| {
                    let dir = dir.clone();
                    async move {
                        Ok::<_, Infallible>(handle(req, &dir).await)
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
        tokio::spawn(async move {
            loop {
                let (stream, _) = match listener.accept().await {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let dir = dir.clone();
                tokio::spawn(async move {
                    let svc = service_fn(move |req: Request<Incoming>| {
                        let dir = dir.clone();
                        async move {
                            Ok::<_, Infallible>(handle(req, &dir).await)
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

async fn handle(req: Request<Incoming>, dir: &Path) -> Response<ResponseBody> {
    let path = req.uri().path().to_string();

    match resolve(dir, &path) {
        Ok(file_path) => {
            match tokio::fs::read(&file_path).await {
                Ok(bytes) => {
                    let mime = mime_type(&file_path);
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
