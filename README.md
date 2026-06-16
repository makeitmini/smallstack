# smallstack

A workspace of independent Rust crates for building small, focused HTTP services. Each crate is minimal, zero-proc-macro, and designed to be used alone or in combination.

| Crate | Purpose | Standalone? |
|---|---|---|
| [`mini-err`](crates/mini-err) | Structured scoped errors with HTTP-friendly codes | Yes |
| [`mini-log`](crates/mini-log) | Structured logger with level filtering and JSON output | Yes |
| [`mini-search`](crates/mini-search) | In-process search engine with BM25 scoring, filters, and field visibility | Yes |
| [`mini-serve`](crates/mini-serve) | HTTP server with router, middleware, CORS, TLS | Yes |
| [`mini-static`](crates/mini-static) | Secure static file server with range requests and live reload | Yes |

## Design

**Small surface area.** Every crate does one thing and one thing well. `mini-err` is an error enum. `mini-log` is a logger. `mini-search` ranks documents. `mini-serve` routes HTTP requests. `mini-static` serves files. None of them is a framework.

**Composable via feature flags.** Supporting crates are optional and gated behind Cargo features (`err`, `log`, `tls`). Using `mini-serve` with `mini-err` is explicit in your `Cargo.toml`, not a transitive surprise.

**No proc macros.** Zero compile-time macro processing. The API is plain types, plain traits, and builder methods.

**Each crate ships independently.** There is no shared runtime, no global state, and no required version alignment across crates. Pick the pieces you need.

### Example: all four together

```toml
[dependencies]
mini-err = "0.3"
mini-log = "0.3"
mini-serve = { version = "0.4", features = ["err", "log"] }
```

```rust,no_run
use mini_serve::{RouteBuilder, handler, json, LoggingMiddleware};
use mini_log::Logger;
use hyper::StatusCode;

let app = RouteBuilder::stateless()
    .wrap(LoggingMiddleware::new(Logger::new("api")).middleware())
    .get("/", handler(|_, _| async {
        json(StatusCode::OK, &serde_json::json!({"ok": true}))
    }))
    .seal();

app.bind("127.0.0.1:3000".parse().unwrap()).await.unwrap();
```

### Example: mini-static standalone

```toml
[dependencies]
mini-static = "0.3"
```

```rust,no_run
use mini_static::Server;
Server::new("./public").bind("0.0.0.0:8080").run().await.unwrap();
```
