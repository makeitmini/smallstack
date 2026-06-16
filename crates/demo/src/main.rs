use hyper::{Request, Response, StatusCode};
use hyper::body::Incoming;
use mini_serve::{handler, json, ResponseBody, RouteBuilder, ServeError, State};
use std::net::SocketAddr;
use tokio::net::TcpListener;

async fn health_check(
    _req: Request<Incoming>,
    _state: State<()>,
) -> Result<Response<ResponseBody>, ServeError> {
    json(StatusCode::OK, &serde_json::json!({"status": "ok"}))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = RouteBuilder::stateless()
        .get("/api/health", handler(health_check))
        .seal();

    let addr: SocketAddr = "0.0.0.0:3000".parse()?;
    let listener = TcpListener::bind(addr).await?;
    mini_serve::bind_with_os_shutdown(listener, app).await?;

    Ok(())
}
