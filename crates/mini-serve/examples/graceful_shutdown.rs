use hyper::{Request, Response, StatusCode};
use hyper::body::Incoming;
use mini_serve::{handler, ResponseBody, RouteBuilder, ServeError, State};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::sleep;

async fn quick_handler(
    _req: Request<Incoming>,
    _state: State<()>,
) -> Result<Response<ResponseBody>, ServeError> {
    mini_serve::json(StatusCode::OK, &serde_json::json!({ "message": "quick response" }))
}

async fn slow_handler(
    _req: Request<Incoming>,
    _state: State<()>,
) -> Result<Response<ResponseBody>, ServeError> {
    // Simulate a slow operation (5 seconds)
    sleep(Duration::from_secs(5)).await;
    mini_serve::json(StatusCode::OK, &serde_json::json!({ "message": "slow response completed" }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app = RouteBuilder::stateless()
        .get("/quick", handler(quick_handler))
        .get("/slow", handler(slow_handler))
        .seal();

    let addr: SocketAddr = "127.0.0.1:3000".parse()?;

    println!("Server listening on {}", addr);
    println!("Try: curl http://localhost:3000/quick");
    println!("Try: curl http://localhost:3000/slow");
    println!("Press Ctrl-C to shut down gracefully");

    // bind_with_os_shutdown handles SIGINT and SIGTERM signals and performs
    // graceful shutdown: stops accepting new connections but allows in-flight
    // requests to complete. Use bind_with_shutdown if you need custom signal logic.
    let listener = TcpListener::bind(addr).await?;
    mini_serve::bind_with_os_shutdown(listener, app).await?;

    println!("Server shut down gracefully");

    Ok(())
}
