# mini-serve

A minimal HTTP server for Rust applications built on hyper, with a routing tree, middleware, CORS, graceful shutdown, optional TLS, and optional integration with mini-err and mini-log — all without proc macros.

```toml
[dependencies]
mini-serve = "0.4"

# Optional: mini-err integration (ServeError from mini_err::Error)
mini-serve = { version = "0.4", features = ["err"] }

# Optional: request logging via mini-log
mini-serve = { version = "0.4", features = ["log"] }

# Optional: TLS termination
mini-serve = { version = "0.4", features = ["tls"] }
```

---

## Philosophy

Most Rust HTTP frameworks offer application builders, extractors, middleware systems, and plug-in ecosystems. Mini-serve does the minimum needed to serve HTTP in production — it wires hyper to a routing trie, then steps out of the way.

### Why not `axum` / `actix-web` / `warp` / `rocket` / `salvo`?

Those frameworks are excellent and far more feature-complete. Mini-serve is for projects that want:

- **A single-file server.** No plug-in system, no custom derive macros, no registry.
- **Explicit ownership.** There is no global state, no async-local spans, no hidden tokio runtime.
- **Predictable concurrency.** Connection limits via semaphore, configurable header read timeout, no implicit thread pools.
- **Transparent error model.** `ServeError` is a struct with `code` and `message`. Handlers return `Result<Response, ServeError>`. That's it.
- **No proc macros.** Zero compile-time dependency on proc-macro crates.

### Design tenets

1. **The builder is the API.** `RouteBuilder::new(state).get("/path", handler).seal()` — every server is an `App` value. No macros, no decorators, no registry.
2. **State is explicit.** Your handler receives `State<S>` (a `Deref` wrapper around `Arc<S>`). There is no global state, no stashing in task-local storage.
3. **Errors are structs with a number and a string.** `ServeError { code: u16, message: String }`. That's all. Convert to HTTP status codes directly.
4. **Middleware wraps handlers.** A middleware is `Arc<dyn Fn(Handler<S>) -> Handler<S>>`. It receives a handler and returns a handler.
5. **Timeouts and limits are built-in.** Path length, query length, body size, header read timeout, and max connections are all configured upfront and enforced before your handler runs.
6. **Graceful shutdown is a first-class concern.** The server drains in-flight connections on SIGINT/SIGTERM before returning.

---

## Usage

### A minimal server

```rust
use mini_serve::{RouteBuilder, handler, json};
use hyper::StatusCode;

#[tokio::main]
async fn main() {
    let app = RouteBuilder::stateless()
        .get("/", handler(|_req, _state| async {
            json(StatusCode::OK, &serde_json::json!({"hello": "world"}))
        }))
        .seal();

    app.bind("127.0.0.1:3000".parse().unwrap())
        .await
        .expect("server failed");
}
```

### Routes

```rust
use mini_serve::RouteBuilder;

let app = RouteBuilder::stateless()
    .get("/", handler(index))
    .post("/users", handler(create_user))
    .put("/users/:id", handler(update_user))
    .delete("/users/:id", handler(delete_user))
    .seal();
```

| Pattern         | Example URL       | Extracted params                                  |
|-----------------|-------------------|---------------------------------------------------|
| `/users/:id`    | `/users/42`       | `{ "id": "42" }`                                  |
| `/files/*`      | `/files/a/b/c`   | `{ "*": "a/b/c" }`                                |
| `/users/:id/posts/:pid` | `/users/1/posts/2` | `{ "id": "1", "pid": "2" }`                |

The router uses a trie with backtracking, matching in precedence order: **static → param → wildcard**.

### Handlers

A handler is `Arc<dyn Fn(Request<Incoming>, State<S>) -> Pin<Box<dyn Future<…>>>>`. Use the `handler()` helper to convert an `async fn`:

```rust
use hyper::{Request, Response, StatusCode};
use hyper::body::Incoming;
use mini_serve::{handler, json, ResponseBody, ServeError, State};

async fn greet(
    _req: Request<Incoming>,
    _state: State<()>,
) -> Result<Response<ResponseBody>, ServeError> {
    json(StatusCode::OK, &serde_json::json!({"msg": "hello"}))
}
```

### State

Pass application state via `RouteBuilder::new(state)`. It becomes available in every handler via `State<S>`, which implements `Deref<Target = S>`:

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use mini_serve::{RouteBuilder, handler};

let counter = Arc::new(AtomicU32::new(0));

let app = RouteBuilder::new(counter.clone())
    .get("/count", handler(|_req, state: State<Arc<AtomicU32>>| async move {
        let val = state.fetch_add(1, Ordering::SeqCst);
        Ok(mini_serve::empty(hyper::StatusCode::OK))
    }))
    .seal();
```

For stateless apps, use `RouteBuilder::stateless()`.

### Path parameters

Path parameters are stored in request extensions as `PathParams(HashMap<String, String>)`:

```rust
use mini_serve::PathParams;

async fn show_user(
    req: Request<Incoming>,
    _state: State<()>,
) -> Result<Response<ResponseBody>, ServeError> {
    let params = req.extensions().get::<PathParams>().cloned().unwrap_or_default();
    let id = params.0.get("id").cloned().unwrap_or_default();
    json(StatusCode::OK, &serde_json::json!({"id": id}))
}
```

For typed extraction, use `path_params()` (requires `serde::Deserialize`):

```rust
use mini_serve::path_params;
use serde::Deserialize;

#[derive(Deserialize)]
struct UserParams {
    id: u32,
}

async fn show_user(req: Request<Incoming>, _state: State<()>) -> Result<Response<ResponseBody>, ServeError> {
    let params: UserParams = path_params(&req)?;
    json(StatusCode::OK, &serde_json::json!({"id": params.id}))
}
```

### Query parameters

Query params are stored in request extensions as `QueryParams(HashMap<String, String>)`:

```rust
use mini_serve::QueryParams;

async fn search(
    req: Request<Incoming>,
    _state: State<()>,
) -> Result<Response<ResponseBody>, ServeError> {
    let params = req.extensions().get::<QueryParams>().cloned().unwrap_or_default();
    let q = params.0.get("q").cloned().unwrap_or_default();
    // …
}
```

### Request body (JSON)

```rust
use mini_serve::json_body;
use serde::Deserialize;

#[derive(Deserialize)]
struct CreateUser {
    name: String,
    email: String,
}

async fn create_user(
    req: Request<Incoming>,
    _state: State<()>,
) -> Result<Response<ResponseBody>, ServeError> {
    let body: CreateUser = json_body(req).await?;
    // …
}
```

Body size is limited to the configured max (default 2 MB). Exceeding it returns **413 Payload Too Large**.

### Response helpers

```rust
use hyper::StatusCode;
use mini_serve::{json, Json, empty, redirect, sse_stream};

// JSON response (function)
json(StatusCode::OK, &serde_json::json!({"key": "value"}))

// JSON response (wrapper)
Json(serde_json::json!({"id": 42})).into_response()?;
Json(serde_json::json!({"id": 42})).into_response_with_status(StatusCode::CREATED)?;

// Empty response with status
empty(StatusCode::NO_CONTENT)

// Redirect (302)
redirect("/new-location")

// SSE stream
use futures_core::Stream;
sse_stream(my_event_stream)
```

### CORS

Configure CORS with the builder:

```rust
use hyper::Method;
use mini_serve::CorsConfig;

let cors = CorsConfig::builder()
    .allow_origin("https://myapp.com")
    .allow_origin("https://admin.myapp.com")
    .allow_method(Method::GET)
    .allow_method(Method::POST)
    .allow_header("Authorization")
    .allow_header("Content-Type")
    .expose_header("X-Request-Id")
    .allow_credentials(true)
    .max_age_secs(3600)
    .build();

let app = RouteBuilder::stateless()
    .with_cors(cors)
    .get("/", handler(handle_index))
    .seal();
```

Preflight (`OPTIONS` with `Origin` header) is handled before routing and returns **204 No Content**. `OPTIONS` without `Origin` passes to normal routing.

CORS with `allow_origin("*")` and `allow_credentials(true)` is rejected: panics in debug builds, disables credentials in release builds with a warning.

### Middleware

```rust
use mini_serve::{middleware, Middleware, Handler};

let m: Middleware<()> = middleware(|handler: Handler<()>| {
    handler(|req, state| async move {
        // Before
        let result = handler(req, state).await;
        // After
        result
    })
});

let app = RouteBuilder::stateless()
    .wrap(m)
    .get("/", handler(my_handler))
    .seal();
```

### Route groups

```rust
let app = RouteBuilder::stateless()
    .group("/api", |group| {
        group
            .get("/users", handler(list_users))
            .post("/users", handler(create_user))
    })
    .seal();
// Mounts: GET /api/users, POST /api/users
```

Groups support `wrap()` for group-level middleware applied to all routes in the group, and nested prefixes.

### Error handling

Handlers return `Result<Response<ResponseBody>, ServeError>`. Unhandled errors are caught by the app's error handler (default: JSON with `message` field). Customize with `with_error_handler()`:

```rust
let app = RouteBuilder::stateless()
    .with_error_handler(|status, message| {
        // status is the HTTP status code from ServeError.code
        // message is ServeError.message
        let body = serde_json::json!({"error": message, "code": status.as_u16()});
        Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(mini_serve::body(hyper::body::Bytes::from(
                serde_json::to_string(&body).unwrap(),
            )))
            .unwrap()
    })
    .get("/", handler(my_handler))
    .seal();
```

### Graceful shutdown

```rust
use tokio::net::TcpListener;

let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
mini_serve::bind_with_os_shutdown(listener, app).await.unwrap();
// Waits for SIGINT or SIGTERM, drains in-flight connections, then returns.
```

Custom shutdown signal:

```rust
use tokio::sync::oneshot;

let (tx, rx) = oneshot::channel();
let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
mini_serve::bind_with_shutdown(listener, app, async { rx.await.unwrap() }).await.unwrap();
```

The server stops accepting new connections on shutdown signal, drains in-flight requests via `JoinSet`, and returns when all handlers complete.

### Configuration

```rust
let app = RouteBuilder::stateless()
    .with_max_body_size(4_194_304)          // 4 MB
    .with_header_read_timeout(Duration::from_secs(60))
    .with_max_connections(512)
    .seal();
```

| Setting                 | Default | Limit                                      |
|-------------------------|---------|--------------------------------------------|
| Max body size           | 2 MB    | Returns 413 if exceeded                    |
| Max path length         | 8192 B  | Returns 400 if exceeded (hard-coded)       |
| Max query string length | 4096 B  | Returns 400 if exceeded (hard-coded)       |
| Header read timeout     | 30 s    | Hyper's idle timeout for header reception  |
| Max connections         | 1024    | Semaphore-acquired permit per connection   |

### Ephemeral port (testing)

```rust
let port = RouteBuilder::stateless()
    .get("/", handler(my_handler))
    .seal()
    .bind_ephemeral()
    .await
    .unwrap();

let resp = reqwest::get(format!("http://localhost:{port}/")).await.unwrap();
```

---

## TLS (optional)

Enable the `tls` feature:

```toml
mini-serve = { version = "0.4", features = ["tls"] }
```

TLS configuration is loaded from environment variables:

```rust
use mini_serve::load_server_config;

let config = load_server_config().expect("failed to load TLS config");
let app = RouteBuilder::stateless()
    .get("/", handler(index))
    .seal();

app.bind_tls("0.0.0.0:443".parse().unwrap(), config).await.unwrap();
```

| Env var              | Description            |
|----------------------|------------------------|
| `SMALLSTACK_TLS_CERT` | Path to PEM cert file  |
| `SMALLSTACK_TLS_KEY`  | Path to PEM key file   |

TLS termination is applied per-accepted-connection before the HTTP handler runs. The same graceful-shutdown infrastructure works for TLS servers.

---

## Mini-err integration (optional)

Enable the `err` feature to convert `mini_err::Error` into `ServeError`:

```toml
mini-serve = { version = "0.4", features = ["err"] }
```

```rust
use mini_err::Error;

fn validate_input(data: &str) -> Result<(), ServeError> {
    if data.is_empty() {
        return Err(Error::bad("api", "input is empty").into());
    }
    Ok(())
}
```

`mini_err::Error` maps its `.code()` to `ServeError.code` and `.message()` to `ServeError.message`.

---

## Request logging (optional)

Enable the `log` feature and attach `LoggingMiddleware`:

```toml
mini-serve = { version = "0.4", features = ["log"] }
```

```rust
use mini_log::Logger;
use mini_serve::LoggingMiddleware;

let logger = Logger::new("http").with_level(mini_log::Level::Info);
let logging = LoggingMiddleware::new(logger);

let app = RouteBuilder::stateless()
    .wrap(logging.middleware())
    .get("/", handler(index))
    .seal();
```

Every request is logged with `method`, `path`, `status`, and `duration` fields. Log level depends on status code: 500+ → `error`, 400+ → `warn`, <400 → `info`.

---

## Architecture

```
TcpListener
  │ accept()
  ▼
semaphore::acquire_owned()    ← max_connections
  │
  ├── (plain) TokioIo(stream)
  └── (TLS)   tokio_rustls::accept() → TokioIo(tls_stream)
  │
  ▼
http1::Builder::serve_connection()
  │ header_read_timeout
  ▼
service_fn(|req| app.route(req))
  │
  ├── path/query length check ← MAX_PATH_LEN / MAX_QUERY_LEN
  ├── CORS preflight (if OPTIONS + Origin) → 204
  ├── router.match_route() → trie with backtracking
  │   ├── found → handler(req, state).await
  │   └── not found → 404 / 405 / HEAD fallback
  ├── CORS headers applied to response
  └── error_handler(status, message) → JSON error response
```

---

## Comparison

| Feature                         | mini-serve | axum | actix-web | warp |
|--------------------------------|-----------|------|-----------|------|
| No proc macros                 | ✓         |      |           | ✓    |
| Explicit state (no globals)    | ✓         | ✓    |           |      |
| Trie router with param capture | ✓         | ✓    | ✓         | ✓    |
| CORS (built-in)                | ✓         |      |           |      |
| Graceful shutdown (built-in)   | ✓         |      |           |      |
| TLS (built-in)                 | opt-in    |      |           | ✓    |
| SSE streams                    | ✓         | ✓    | ✓         | ✓    |
| Async middleware               | ✓         | ✓    | ✓         | ✓    |
| Tower service compat           |           | ✓    |           |      |
| WebSockets                     |           | ✓    | ✓         | ✓    |
| Multipart forms                |           | ✓    | ✓         |      |

Mini-serve is best suited for applications that want a predictable, minimal HTTP server without adopting a framework's full ecosystem. If you need WebSockets, multipart, Tower ecosystem compatibility, or async extractors, consider `axum`. If you need an actor model or real-time WebSocket support, consider `actix-web`. Mini-serve does one thing — serve HTTP — and delegates everything else to your code.

---

## MSRV

The minimum supported Rust version is **1.75**. Bumping the MSRV is considered a breaking change.
