# mini-static

A minimal, secure static file server for Rust applications built on hyper, with directory indexes, conditional GET, range requests, custom handlers, content transforms, and optional live reload — all without proc macros.

```toml
[dependencies]
mini-static = "0.3"

# Optional: mini-err integration (StaticError → mini_err::Error)
mini-static = { version = "0.3", features = ["err"] }

# Optional: request logging via mini-log
mini-static = { version = "0.3", features = ["log"] }
```

---

## Philosophy

Serving static files sounds trivial — read a file, write a response. Doing it *safely* requires path traversal protection, correct MIME types, range request handling, conditional GETs, and connection management. Mini-static packages all of that into a single builder, then gets out of your way.

### Why not `axum` / `actix-web` / `warp` / `nginx`?

Mini-static does one thing: serve a directory of files over HTTP. It is not a general-purpose HTTP framework, nor a production reverse proxy. It is for:

- **Development servers.** Live reload is built-in (debug builds only).
- **Embedded file serving.** Ship a binary that also serves your frontend.
- **Test fixtures.** `run_ephemeral()` returns a port for integration tests.
- **Simple deployments.** A single static binary with no nginx dependency.

### Design tenets

1. **Security is not optional.** Path traversal via `..`, null bytes, percent-encoding, symlinks, and double encoding are all blocked by the `resolve()` function. Directory listing is never generated — only `index.html` is served.
2. **Streaming by default.** Files are read in 64 KB chunks. A full buffered read is used only when a transform or live reload is active.
3. **HTTP semantics are respected.** ETag/If-None-Match, Last-Modified/If-Modified-Since, Range/If-Range, and HEAD are handled correctly.
4. **Custom handlers come first.** Before any file is read, registered handlers get a chance to intercept the request. This lets you embed API endpoints or middleware.
5. **No proc macros.** `Handler` and `Transform` are plain traits with no derive macros.

---

## Usage

### A minimal server

```rust
use mini_static::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Server::new("./public")
        .bind("127.0.0.1:3000")
        .run()
        .await?;
    Ok(())
}
```

Serves files from `./public`. Directory `index.html` files are served automatically. Missing files return **404**, path traversal attempts return **403**, I/O errors return **500**.

### Configuration

```rust
use mini_static::Server;

let srv = Server::new("./public")
    .bind("0.0.0.0:8080")           // default: 127.0.0.1:0
    .with_max_connections(512);     // default: 1024

srv.run().await?;
```

### Ephemeral port (testing)

```rust
let port = Server::new("./public")
    .run_ephemeral()
    .await?;

let resp = reqwest::get(format!("http://localhost:{port}/index.html")).await?;
```

### Graceful shutdown

```rust
use tokio::net::TcpListener;

let listener = TcpListener::bind("127.0.0.1:3000").await?;
Server::new("./public")
    .run_with_os_shutdown(listener)
    .await?;
// Waits for SIGINT or SIGTERM, drains in-flight connections, then returns.
```

Custom shutdown signal:

```rust
let listener = TcpListener::bind("127.0.0.1:3000").await?;
let (tx, rx) = tokio::sync::oneshot::channel::<()>();

tokio::spawn(async move {
    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    let _ = tx.send(());
});

Server::new("./public")
    .run_with_shutdown(listener, async { rx.await.unwrap(); })
    .await?;
```

---

## Path resolution

```
resolve("/var/www", "/")              → /var/www/index.html   (if exists)
resolve("/var/www", "/index.html")    → /var/www/index.html
resolve("/var/www", "/sub/")          → /var/www/sub/index.html
resolve("/var/www", "/../etc/passwd") → Err(Traversal)       ← 403
resolve("/var/www", "/nonexistent")   → Err(NotFound)         ← 404
```

The resolver:

- Decodes percent-encoded characters (ASCII-only; multi-byte sequences like `%C3%A9` produce replacement characters and fail canonicalization)
- Rejects null bytes (`\0`)
- Rejects paths containing `..` (raw or percent-encoded)
- Canonicalizes the joined path and verifies it starts with the root directory
- Blocks symlink escapes (symlinks pointing outside root are rejected)
- Falls back to `index.html` for directories

---

## Handlers

Register custom handlers that run before static file resolution. If a handler returns `Some(response)`, it is used; if `None`, the request falls through to static file serving.

```rust
use std::convert::Infallible;
use std::pin::Pin;
use mini_static::{Handler, RequestInfo, ResponseBody, Server};
use hyper::body::Bytes;
use hyper::Response;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};

struct PingHandler;

impl Handler for PingHandler {
    fn handle(
        &self,
        info: RequestInfo,
    ) -> Pin<Box<dyn Future<Output = Option<Response<ResponseBody>>> + Send + '_>> {
        Box::pin(async move {
            if info.path == "/api/ping" {
                let b = BoxBody::new(
                    Full::new(Bytes::from("pong"))
                        .map_err(|e: Infallible| match e {}),
                );
                Some(Response::builder()
                    .status(200)
                    .header("content-type", "text/plain")
                    .body(b)
                    .unwrap())
            } else {
                None
            }
        })
    }
}

Server::new("./public")
    .with_handler(PingHandler)
    .run()
    .await?;
```

Multiple handlers are called in registration order. The first that returns `Some` wins.

---

## Transforms

A `Transform` is called on every file response after the file is fully buffered (not during streaming responses). This enables template injection, string replacement, or content modification.

```rust
use mini_static::Server;

Server::new("./public")
    .with_transform(|content_type: &str, body: Vec<u8>| {
        if content_type == "text/html" {
            let mut out = b"<!-- served by mini-static -->\n".to_vec();
            out.extend(body);
            out
        } else {
            body
        }
    })
    .run()
    .await?;
```

Transforms receive the MIME type and the full file body. They return the modified body. The `Transform` trait is implemented for all `Fn(&str, Vec<u8>) -> Vec<u8>` closures.

Files are buffered when a transform is present (or in debug builds with live reload). Otherwise, files are streamed in 64 KB chunks.

---

## Caching and conditional GET

Every response includes:

| Header           | Value                              |
|------------------|------------------------------------|
| `etag`           | `W/"{mtime_secs}-{file_len}"`      |
| `last-modified`  | HTTP-date formatted mtime          |
| `cache-control`  | `no-cache`                         |
| `accept-ranges`  | `bytes`                            |

If the client sends `If-None-Match` (matching the ETag) or `If-Modified-Since` (file not newer), the server returns **304 Not Modified** with an empty body.

```rust
// Client:
GET /style.css
→ 200 + ETag: W/"1712345678-1234"

// Client with cached ETag:
GET /style.css
If-None-Match: W/"1712345678-1234"
→ 304 Not Modified (empty body)
```

---

## Range requests

Single `bytes=` range requests are supported. Multi-part ranges (`bytes=0-2,5-7`) are rejected with **416 Range Not Satisfiable**.

```http
GET /video.mp4
Range: bytes=1000-1999

→ 206 Partial Content
Content-Range: bytes 1000-1999/50000
```

| Range format          | Example        | Behavior                             |
|-----------------------|----------------|--------------------------------------|
| `bytes={start}-{end}` | `bytes=0-999`  | Specified inclusive range            |
| `bytes={start}-`      | `bytes=1024-`  | From start to end of file            |
| `bytes=-{n}`          | `bytes=-100`   | Last N bytes of file                 |
| `bytes={start}-{end}` | `bytes=99999-` | Past end → 416 Unsatisfiable         |
| Multi-part            | `bytes=0-1,2-3`| Not supported → 416 Unsatisfiable    |

Files are seeked to the range start and streamed from there. Range requests are satisfied from the streaming path only (transforms force full buffering, which disables range-seeking).

---

## MIME types

MIME types are detected via `mime_guess`. Unknown extensions fall back to `application/octet-stream`.

```rust
use mini_static::mime_type;
use std::path::Path;

assert_eq!(mime_type(Path::new("index.html")), "text/html");
assert_eq!(mime_type(Path::new("style.css")),  "text/css");
assert_eq!(mime_type(Path::new("data.zzz")),   "application/octet-stream");
```

---

## Live reload (debug builds only)

In debug builds, the server injects a live-reload script into all HTML responses and exposes an SSE endpoint at `/__mini_reload`:

- The server polls the file directory every 500 ms for changes.
- On change, an SSE event is sent to all connected browsers.
- CSS changes reload stylesheets in-place (no full page reload).
- HTML, JS, and other changes trigger a full page reload.
- The script is injected before `</body>` (or `</html>` if no `</body>` is found).

No configuration is needed — it is automatic in debug builds. In release builds, live reload is compiled out entirely (zero overhead).

---

## Architecture

```
Server::new(dir).run()
  │
  ├── canonicalize root directory
  ├── (debug) setup_livereload: start file poller + SSE handler
  │
  ▼
serve_inner(listener, dir, handlers, transform, log, max_connections)
  │
  ├── semaphore::acquire_owned()    ← max_connections
  │
  ▼
tokio::spawn(connection_handler)
  │
  ▼
service_fn(|req| → Response)
  │
  ├── handlers.iter()              ← custom Handler trait
  │   └── match → Some(resp)      → return early
  │
  ├── resolve(dir, path)           ← traversal guard
  │   ├── Err(Traversal)           → 403
  │   ├── Err(NotFound)            → 404
  │   └── Ok(file_path)
  │
  ├── metadata → ETag + Last-Modified
  │
  ├── is_not_modified()            → 304
  │
  ├── parse_range() → 206 / 416 / full
  │
  ├── needs_buffer? (transform || live_reload)
  │   ├── yes: fs::read → transform → response
  │   └── no:  fs::File → ReadStream → streaming response
  │
  └── log(method, path, status)    ← optional mini-log
```

---

## Error types

`StaticError` is a three-variant enum:

| Variant      | HTTP code | `user_message()`           | `to_string()`               |
|-------------|-----------|----------------------------|-----------------------------|
| `NotFound`  | 404       | `"not found"`              | `"not found: {path}"`       |
| `Traversal` | 403       | `"path traversal denied"`  | `"path traversal denied: {path}"` |
| `Io`        | 500       | `"internal server error"`  | `"io error: {e}"`           |

`user_message()` never leaks internal paths or OS error details to the client.

### Mini-err integration (optional)

Enable the `err` feature to convert `StaticError` into `mini_err::Error`:

```toml
mini-static = { version = "0.3", features = ["err"] }
```

```rust
use mini_static::StaticError;

let err = StaticError::NotFound("/missing.txt".into());
let mini: mini_err::Error = err.into();
assert_eq!(mini.code(), 404);
assert_eq!(mini.scope(), "static");
```

| StaticError     | mini_err variant | Code |
|-----------------|------------------|------|
| `NotFound`      | `Gone`           | 404  |
| `Traversal`     | `Bad`            | 400  |
| `Io`            | `Io`             | 500  |

### Request logging (optional)

Enable the `log` feature:

```toml
mini-static = { version = "0.3", features = ["log"] }
```

```rust
use mini_static::Server;

let logger = mini_log::Logger::new("serve").with_level(mini_log::Level::Info);

let port = Server::new("./public")
    .with_logger(logger)
    .run_ephemeral()
    .await?;
```

Every request is logged with `method`, `path`, and `status` fields.

---

## Comparison

| Feature                         | mini-static | axum static | actix-files | nginx |
|--------------------------------|-------------|-------------|-------------|-------|
| No proc macros                 | ✓           |             |             | —     |
| Path traversal protection      | ✓           | varies      | varies      | ✓     |
| Directory index (index.html)   | ✓           | opt-in      | opt-in      | ✓     |
| Conditional GET (304)          | ✓           | varies      | ✓           | ✓     |
| Range requests (206)           | ✓           |             | ✓           | ✓     |
| Custom handlers before files   | ✓           | ✓           | ✓           |       |
| Content transforms             | ✓           | ✓           |             |       |
| MIME type detection            | ✓           | ✓           | ✓           | ✓     |
| Live reload (dev mode)         | ✓           |             |             |       |
| Graceful shutdown              | ✓           | ✓           | ✓           | ✓     |
| Connection limiting            | ✓           | ✓           | ✓           | ✓     |

Mini-static is best suited for development servers, embedded file serving, and test fixtures — anywhere you need a safe, zero-config static file server in a Rust binary. For production-grade serving with caching proxies, compression, and HTTP/2, use `nginx` or `Caddy` in front.

---

## MSRV

The minimum supported Rust version is **1.75**. Bumping the MSRV is considered a breaking change.
