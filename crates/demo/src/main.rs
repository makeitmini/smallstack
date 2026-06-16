use std::net::SocketAddr;
use std::time::Instant;

use hyper::{Request, Response, StatusCode};
use hyper::body::Incoming;
use mini_log::Logger;
use mini_serve::{handler, json, ResponseBody, RouteBuilder, ServeError, State};
use tokio::net::TcpListener;

#[derive(Clone)]
struct AppState {
    logger: Logger,
    started_at: Instant,
}

impl AppState {
    fn new(logger: Logger) -> Self {
        Self { logger, started_at: Instant::now() }
    }
}

async fn health_check(
    _req: Request<Incoming>,
    state: State<AppState>,
) -> Result<Response<ResponseBody>, ServeError> {
    let uptime = state.started_at.elapsed().as_secs();
    state.logger.info("health_check")
        .field("uptime_secs", uptime)
        .emit();
    json(StatusCode::OK, &serde_json::json!({
        "status": "ok",
        "uptime_secs": uptime,
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let logger = Logger::from_env("demo");

    logger.info("server_starting")
        .field("version", env!("CARGO_PKG_VERSION"))
        .emit();

    let state = AppState::new(logger.clone());

    let app = RouteBuilder::new(state)
        .get("/api/health", handler(health_check))
        .seal();

    let addr: SocketAddr = "0.0.0.0:3000".parse()?;

    logger.info("server_listening")
        .field("addr", addr.to_string())
        .emit();

    let listener = TcpListener::bind(addr).await?;
    mini_serve::bind_with_os_shutdown(listener, app).await?;

    logger.info("server_stopped").emit();

    Ok(())
}
