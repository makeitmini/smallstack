use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime};

use futures_core::Stream;
use hyper::body::{Bytes, Frame, Incoming};
use hyper::header::HeaderMap;
use hyper::{Request, Response, StatusCode};
use hyper::service::service_fn;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use hyper_util::server::conn::auto::Builder as AutoBuilder;
use http_body::Body;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full, StreamBody};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, ReadBuf};
use tokio::net::TcpListener;
use tokio::task::JoinSet;

use crate::error::StaticError;

/// Wraps any body into `BoxBody<Bytes, StaticError>`, converting errors via
/// `Into<StaticError>` (which covers `Infallible` and `StaticError` itself).
pub fn into_body<B>(body: B) -> BoxBody<Bytes, StaticError>
where
    B: Body<Data = Bytes> + Send + Sync + 'static,
    B::Error: Into<StaticError>,
{
    BoxBody::new(body.map_err(Into::into))
}

/// Chunk size for streaming file reads — 64 KB per frame.
const FILE_CHUNK_SIZE: usize = 65_536;

/// Default maximum concurrent connections.
const DEFAULT_MAX_CONNECTIONS: usize = 1024;

/// Adapter that converts a `tokio::fs::File` (or any `AsyncRead`) into a
/// `Stream` of `Frame<Bytes>` values, yielding one chunk per read.
struct ReadStream<R> {
    reader: R,
    buf: Vec<u8>,
}

fn read_classification(io_result: &std::io::Result<()>, n: usize) -> &'static str {
    match io_result {
        Ok(()) if n == 0 => "eof",
        Ok(()) => "chunk",
        Err(_) => "error",
    }
}

impl<R: AsyncRead + Unpin> Stream for ReadStream<R> {
    type Item = Result<Frame<Bytes>, StaticError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        this.buf.resize(FILE_CHUNK_SIZE, 0);
        let mut read_buf = ReadBuf::new(&mut this.buf);
        let reader = Pin::new(&mut this.reader);
        let poll = reader.poll_read(cx, &mut read_buf);
        match poll {
            Poll::Ready(result) => {
                let n = read_buf.filled().len();
                match read_classification(&result, n) {
                    "eof" => Poll::Ready(None),
                    "chunk" => {
                        let chunk = Bytes::copy_from_slice(&this.buf[..n]);
                        Poll::Ready(Some(Ok(Frame::data(chunk))))
                    }
                    "error" => {
                        let err = result.unwrap_err();
                        eprintln!("[mini-static] read error: {err}");
                        Poll::Ready(Some(Err(StaticError::Io(err))))
                    }
                    _ => unreachable!(),
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

use crate::handler::{Handler, RequestInfo, ResponseBody};
#[cfg(debug_assertions)]
use crate::live;
use crate::mime::mime_type;
use crate::resolve::resolve;
use crate::transform::Transform;

type LogFn = Arc<dyn Fn(&str, &str, u16) + Send + Sync>;

pub struct Server {
    dir:            Arc<PathBuf>,
    addr:           SocketAddr,
    handlers:       Vec<Arc<dyn Handler>>,
    transform:      Option<Arc<dyn Transform>>,
    max_connections: usize,
    #[cfg(feature = "log")]
    logger:         Option<Arc<mini_log::Logger>>,
}

impl Server {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Server {
            dir:             Arc::new(dir.into()),
            addr:            ([127, 0, 0, 1], 0).into(),
            handlers:        Vec::new(),
            transform:       None,
            max_connections: DEFAULT_MAX_CONNECTIONS,
            #[cfg(feature = "log")]
            logger:          None,
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

    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }

    #[cfg(feature = "log")]
    pub fn with_logger(mut self, logger: mini_log::Logger) -> Self {
        self.logger = Some(Arc::new(logger));
        self
    }

    pub async fn run(mut self) -> Result<(), StaticError> {
        let max_connections = self.max_connections;
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
        serve_inner(listener, dir, handlers, transform, log, max_connections).await;
        Ok(())
    }

    pub async fn run_ephemeral(mut self) -> Result<u16, StaticError> {
        let max_connections = self.max_connections;
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
            serve_inner(listener, dir, handlers, transform, log, max_connections).await;
        });
        Ok(port)
    }

    pub async fn run_with_shutdown<F>(mut self, listener: TcpListener, shutdown: F) -> Result<(), StaticError>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let max_connections = self.max_connections;
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
        serve_with_shutdown(listener, dir, handlers, transform, log, max_connections, shutdown).await;
        Ok(())
    }

    pub async fn run_with_os_shutdown(self, listener: TcpListener) -> Result<(), StaticError> {
        self.run_with_shutdown(listener, signal_shutdown()).await
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
    max_connections: usize,
) {
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_connections));
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(_) => continue,
        };
        let sem = semaphore.clone();
        let permit = sem.acquire_owned().await;
        let dir = dir.clone();
        let handlers = handlers.clone();
        let transform = transform.clone();
        let log = log.clone();
        tokio::spawn(async move {
            let _permit = permit.expect("semaphore closed");
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

async fn signal_shutdown() {
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .expect("failed to set up SIGINT handler");
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("failed to set up SIGTERM handler");

    tokio::select! {
        _ = sigint.recv() => {},
        _ = sigterm.recv() => {},
    }
}

async fn serve_with_shutdown<F>(
    listener: TcpListener,
    dir: Arc<PathBuf>,
    handlers: Arc<Vec<Arc<dyn Handler>>>,
    transform: Option<Arc<dyn Transform>>,
    log: Option<LogFn>,
    max_connections: usize,
    shutdown: F,
) where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_connections));
    let mut join_set: JoinSet<()> = JoinSet::new();

    let mut shutdown_pin = std::pin::pin!(shutdown);
    let mut shutdown_initiated = false;

    loop {
        tokio::select! {
            result = listener.accept(), if !shutdown_initiated => {
                let (stream, _) = match result {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let sem = semaphore.clone();
                let permit = match sem.acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let dir = dir.clone();
                let handlers = handlers.clone();
                let transform = transform.clone();
                let log = log.clone();
                let join_set_ref = &mut join_set;
                join_set_ref.spawn(async move {
                    let _permit = permit;
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
            _ = &mut shutdown_pin, if !shutdown_initiated => {
                shutdown_initiated = true;
            }
            Some(_) = join_set.join_next(), if shutdown_initiated => {
                // One task completed, continue draining.
            }
            else => {
                // All tasks drained, shutdown complete.
                break;
            }
        }
    }
}

enum RangeOutcome {
    None,
    Satisfiable(u64, u64), // inclusive start, inclusive end
    Unsatisfiable,
}

fn parse_range_spec(spec: &str, file_len: u64) -> Option<(u64, u64)> {
    let (s, e) = spec.split_once('-')?;
    match (s.trim(), e.trim()) {
        ("", "") => None,
        ("", n) => {
            let n: u64 = n.parse().ok()?;
            if n == 0 || file_len == 0 {
                return None;
            }
            Some((file_len.saturating_sub(n), file_len - 1))
        }
        (s, "") => {
            let start: u64 = s.parse().ok()?;
            if start >= file_len {
                return None;
            }
            Some((start, file_len - 1))
        }
        (s, e) => {
            let start: u64 = s.parse().ok()?;
            let end: u64 = e.parse().ok()?;
            if start > end || start >= file_len {
                return None;
            }
            Some((start, end.min(file_len - 1)))
        }
    }
}

fn parse_range(headers: &HeaderMap, file_len: u64) -> RangeOutcome {
    let Some(val) = headers.get("range") else {
        return RangeOutcome::None;
    };
    let Ok(s) = val.to_str() else {
        return RangeOutcome::None;
    };
    let Some(spec) = s.strip_prefix("bytes=") else {
        return RangeOutcome::None;
    };
    if spec.contains(',') {
        return RangeOutcome::Unsatisfiable;
    }
    match parse_range_spec(spec, file_len) {
        Some((start, end)) => RangeOutcome::Satisfiable(start, end),
        None => RangeOutcome::Unsatisfiable,
    }
}

fn weak_eq(a: &str, b: &str) -> bool {
    fn strip(s: &str) -> &str { s.strip_prefix("W/").unwrap_or(s) }
    strip(a.trim()) == strip(b.trim())
}

fn is_not_modified(headers: &HeaderMap, etag: &str, mtime: SystemTime) -> bool {
    if let Some(inm) = headers.get("if-none-match") {
        let v = inm.to_str().unwrap_or("");
        return v == "*" || v.split(',').any(|c| weak_eq(c, etag));
    }
    if let Some(ims) = headers.get("if-modified-since") {
        if let Ok(s) = ims.to_str() {
            if let Ok(t) = httpdate::parse_http_date(s) {
                return mtime <= t;
            }
        }
    }
    false
}

pub async fn handle_request(
    req: Request<Incoming>,
    dir: &Path,
    handlers: &[Arc<dyn Handler>],
    transform: Option<Arc<dyn Transform>>,
) -> Response<BoxBody<Bytes, Infallible>> {
    let resp = handle(req, dir, handlers, &transform).await;
    let (parts, body) = resp.into_parts();
    let bytes = body
        .collect()
        .await
        .map(|collected| collected.to_bytes())
        .unwrap_or_else(|_| Bytes::new());
    let mut resp = Response::new(BoxBody::new(Full::new(bytes)));
    *resp.status_mut() = parts.status;
    *resp.headers_mut() = parts.headers;
    resp
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
            let mut cache_headers: Option<(String, String)> = None;

            if let Ok(meta) = tokio::fs::metadata(&file_path).await {
                let mtime_raw = meta.modified().unwrap_or(SystemTime::now());
                let mtime_secs = mtime_raw.duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                // Reconstruct mtime from whole seconds to match httpdate precision
                let mtime = SystemTime::UNIX_EPOCH + Duration::from_secs(mtime_secs);
                let etag = format!(r#"W/"{}-{}""#, mtime_secs, meta.len());
                let last_modified = httpdate::fmt_http_date(mtime);

                if is_not_modified(req.headers(), &etag, mtime) {
                    return Response::builder()
                        .status(StatusCode::NOT_MODIFIED)
                        .header("etag", &etag)
                        .header("cache-control", "no-cache")
                        .body(into_body(Full::new(Bytes::new())))
                        .unwrap();
                }

                cache_headers = Some((etag, last_modified));

                let file_len = meta.len();
                let needs_buffer = transform.is_some()
                    || (cfg!(debug_assertions) && mime.starts_with("text/html"));

                match parse_range(req.headers(), file_len) {
                    RangeOutcome::Unsatisfiable => {
                        return Response::builder()
                            .status(StatusCode::RANGE_NOT_SATISFIABLE)
                            .header("content-range", format!("bytes */{file_len}"))
                            .body(into_body(Full::new(Bytes::new())))
                            .unwrap();
                    }
                    RangeOutcome::Satisfiable(start, end) if !needs_buffer => {
                        match tokio::fs::File::open(&file_path).await {
                            Ok(mut file) => {
                                if let Err(_) = file.seek(std::io::SeekFrom::Start(start)).await {
                                    return error_response(
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        "seek failed",
                                    );
                                }
                                let length = end - start + 1;
                                let limited = file.take(length);
                                let stream =
                                    ReadStream { reader: limited, buf: Vec::with_capacity(FILE_CHUNK_SIZE) };
                                let body = into_body(StreamBody::new(stream));
                                let mut resp = Response::builder()
                                    .status(StatusCode::PARTIAL_CONTENT)
                                    .header("content-type", mime)
                                    .header("content-range", format!("bytes {start}-{end}/{file_len}"))
                                    .header("content-length", length.to_string())
                                    .header("accept-ranges", "bytes");
                                if let Some((etag, lm)) = &cache_headers {
                                    resp = resp
                                        .header("etag", etag)
                                        .header("last-modified", lm)
                                        .header("cache-control", "no-cache");
                                }
                                return resp.body(body).unwrap();
                            }
                            Err(_) => {
                                return error_response(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "failed to read file",
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }

            let needs_buffer = transform.is_some()
                || (cfg!(debug_assertions) && mime.starts_with("text/html"));
            if needs_buffer {
                match tokio::fs::read(&file_path).await {
                    Ok(mut bytes) => {
                        if let Some(ref t) = transform {
                            bytes = t.apply(mime, bytes);
                        }
                        file_response(bytes, mime, cache_headers.as_ref().map(|(e, l)| (e.as_str(), l.as_str())))
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
                        let body = into_body(StreamBody::new(stream));
                        let mut resp = Response::builder()
                            .status(StatusCode::OK)
                            .header("content-type", mime)
                            .header("accept-ranges", "bytes");
                        if let Some((etag, last_modified)) = &cache_headers {
                            resp = resp
                                .header("etag", etag)
                                .header("last-modified", last_modified)
                                .header("cache-control", "no-cache");
                        }
                        resp
                            .body(body)
                            .unwrap_or_else(|_| {
                                // Fallback to a static 500 response if header construction fails
                                Response::builder()
                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                    .body(into_body(Full::new(Bytes::from(
                                        r#"{"message":"internal server error"}"#.to_string()
                                    ))))
                                    .unwrap_or_else(|_| {
                                        // Even if the fallback building fails, return a hardcoded response
                                        Response::builder()
                                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                                            .body(into_body(Full::new(Bytes::from(
                                                r#"{"message":"internal server error"}"#.to_string()
                                            ))))
                                            .unwrap()
                                    })
                            })
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
        .body(into_body(Full::new(Bytes::from(json))))
        .unwrap_or_else(|_| {
            // Fallback to a static 500 response if header construction fails
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(into_body(Full::new(Bytes::from(
                    r#"{"message":"internal server error"}"#.to_string()
                ))))
                .unwrap_or_else(|_| {
                    // Even if the fallback building fails, return a hardcoded response
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(into_body(Full::new(Bytes::from(
                            r#"{"message":"internal server error"}"#.to_string()
                        ))))
                        .unwrap()
                })
        })
}

#[cfg(not(debug_assertions))]
fn file_response(bytes: Vec<u8>, mime: &str, cache: Option<(&str, &str)>) -> Response<ResponseBody> {
    let mut resp = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", mime)
        .header("accept-ranges", "bytes");
    if let Some((etag, last_modified)) = cache {
        resp = resp
            .header("etag", etag)
            .header("last-modified", last_modified)
            .header("cache-control", "no-cache");
    }
    resp
        .body(into_body(Full::new(Bytes::from(bytes))))
        .unwrap_or_else(|_| {
            // Fallback to a static 500 response if header construction fails
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(into_body(Full::new(Bytes::from(
                    r#"{"message":"internal server error"}"#.to_string()
                ))))
                .unwrap_or_else(|_| {
                    // Even if the fallback building fails, return a hardcoded response
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(into_body(Full::new(Bytes::from(
                            r#"{"message":"internal server error"}"#.to_string()
                        ))))
                        .unwrap()
                })
        })
}

#[cfg(debug_assertions)]
fn file_response(bytes: Vec<u8>, mime: &str, _cache: Option<(&str, &str)>) -> Response<ResponseBody> {
    crate::live::make_html_response(bytes, mime)
}
