use mini_static::Server;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let srv = Server::new("./public");
    let addr: SocketAddr = "127.0.0.1:3000".parse()?;
    let listener = TcpListener::bind(addr).await?;

    println!("Serving ./public on {}", addr);
    println!("Try: curl http://localhost:3000/");
    println!("Press Ctrl-C to shut down gracefully");

    // run_with_os_shutdown handles SIGINT and SIGTERM signals and performs
    // graceful shutdown: stops accepting new connections but allows in-flight
    // requests to complete. Use run_with_shutdown if you need custom signal logic.
    srv.run_with_os_shutdown(listener).await?;

    println!("Server shut down gracefully");

    Ok(())
}
