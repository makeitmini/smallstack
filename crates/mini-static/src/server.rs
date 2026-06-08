use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures_core::Stream;
use hyper::body::{Bytes, Frame, Incoming};
use hyper::{Request, Response, StatusCode};
use hyper::service::service_fn;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use hyper_util::server::conn::auto::Builder as AutoBuilder;
use http_body_util::combinators::BoxBody;
use http_body_util::{Full, StreamBody};
use tokio::io::{AsyncRead, ReadBuf};
use tokio::net::TcpListener;

/// Chunk size for streaming file reads — 64 KB per frame.
const FILE_CHUNK_SIZE: usize = 65_536;

/// Adapter that converts a `tokio::fs::File` (or any `AsyncRead`) into a
/// `Stream` of `Frame<Bytes>` values, yielding one chunk per read.
struct ReadStream<R> {
    reader: R,
    buf: Vec<u8>,
}

impl<R: AsyncRead + Unpin> Stream for ReadStream<R> {
    type Item = Result<Frame<Bytes>, Infallible>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        this.buf.resize(FILE_CHUNK_SIZE, 0);
        let mut read_buf = ReadBuf::new(&mut this.buf);
        let reader = Pin::new(&mut this.reader);
        match reader.poll_read(cx, &mut read_buf) {
            Poll::Ready(Ok(())) => {
                let n = read_buf.filled().len();
                if n == 0 {
                    Poll::Ready(None)
                } else {
                    let chunk = Bytes::copy_from_slice(&this.buf[..n]);
                    Poll::Ready(Some(Ok(Frame::data(chunk))))
                }
            }
            Poll::Ready(Err(_)) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

use crate::error::StaticError;
use crate::handler::{Handler, RequestInfo, ResponseBody};
#[cfg(debug_assertions)]
use crate::live;
use crate::mime::mime_type;
use crate::resolve::resolve;
use crate::transform::Transform;

type LogFn = Arc<dyn Fn(&str, &str, u16) + Send + Sync>;

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

    pub async fn run(mut self) -> Result<(), StaticError> {
        let listener = TcpListener::bind(self.addr)
            .await
            .map_err(StaticError::Io)?;
        let dir = self.dir.canonicalize().map_err(StaticError::Io)?;
        let dir = Arc::new(dir);
        #[cfg(debug_assertions)]
        self.setup_livereload(&dir);
        let handlers = Arc::new(self.handlers);
        let transform = self.transform;
        #[cfg(feature = "log")]
        let log = self.logger.map(|l| {
            Arc::new(move |method: &str, path: &str, status: u16| {
                l.info("serve")
                    .field("method", method)
                    .field("path", path)
                    .field("status", status)
                    .emit();
            }) as LogFn
        });
        #[cfg(not(feature = "log"))]
        let log = None::<LogFn>;
        serve_inner(listener, dir, handlers, transform, log).await;
        Ok(())
    }

    pub async fn run_ephemeral(mut self) -> Result<u16, StaticError> {
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
        #[cfg(debug_assertions)]
        self.setup_livereload(&dir);
        let handlers = Arc::new(self.handlers);
        let transform = self.transform;
        #[cfg(feature = "log")]
        let log = self.logger.map(|l| {
            Arc::new(move |method: &str, path: &str, status: u16| {
                l.info("serve")
                    .field("method", method)
                    .field("path", path)
                    .field("status", status)
                    .emit();
            }) as LogFn
        });
        #[cfg(not(feature = "log"))]
        let log = None::<LogFn>;
        tokio::spawn(async move {
            serve_inner(listener, dir, handlers, transform, log).await;
        });
        Ok(port)
    }

    #[cfg(debug_assertions)]
    fn setup_livereload(&mut self, dir: &Arc<PathBuf>) {
        let broadcaster = live::Broadcaster::new();
        self.handlers
            .push(Arc::new(live::SseHandler { broadcaster: broadcaster.clone() }));
        live::start_poller(dir.clone(), broadcaster);
    }
}

async fn serve_inner(
    listener: TcpListener,
    dir: Arc<PathBuf>,
    handlers: Arc<Vec<Arc<dyn Handler>>>,
    transform: Option<Arc<dyn Transform>>,
    log: Option<LogFn>,
) {
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };
        let dir = dir.clone();
        let handlers = handlers.clone();
        let transform = transform.clone();
        let log = log.clone();
        tokio::spawn(async move {
            let svc = service_fn(move |req: Request<Incoming>| {
                let dir = dir.clone();
                let handlers = handlers.clone();
                let transform = transform.clone();
                let log = log.clone();
                async move {
                    let method = req.method().to_string();
                    let path = req.uri().path().to_string();
                    let resp = handle(req, &dir, &handlers, &transform).await;
                    let status = resp.status().as_u16();
                    if let Some(ref log) = log {
                        log(&method, &path, status);
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
            let mime = mime_type(&file_path);
            // Buffered path: required when a transform is registered (operates
            // on Vec<u8>) or when livereload injection needs full HTML in debug
            // builds. Otherwise stream for constant memory per connection.
            let needs_buffer = transform.is_some()
                || (cfg!(debug_assertions) && mime.starts_with("text/html"));
            if needs_buffer {
                match tokio::fs::read(&file_path).await {
                    Ok(mut bytes) => {
                        if let Some(ref t) = transform {
                            bytes = t.apply(mime, bytes);
                        }
                        file_response(bytes, mime)
                    }
                    Err(_) => error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to read file",
                    ),
                }
            } else {
                match tokio::fs::File::open(&file_path).await {
                    Ok(file) => {
                        let stream = ReadStream { reader: file, buf: Vec::with_capacity(FILE_CHUNK_SIZE) };
                        let body = BoxBody::new(StreamBody::new(stream));
                        Response::builder()
                            .status(StatusCode::OK)
                            .header("content-type", mime)
                            .body(body)
                            .unwrap()
                    }
                    Err(_) => error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to read file",
                    ),
                }
            }
        }
        Err(e) => {
            let status = e.status_code();
            let msg = e.user_message();
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

#[cfg(not(debug_assertions))]
fn file_response(bytes: Vec<u8>, mime: &str) -> Response<ResponseBody> {
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", mime)
        .body(BoxBody::new(Full::new(Bytes::from(bytes))))
        .unwrap()
}

#[cfg(debug_assertions)]
fn file_response(bytes: Vec<u8>, mime: &str) -> Response<ResponseBody> {
    crate::live::make_html_response(bytes, mime)
}
