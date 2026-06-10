# mini-err

A minimal, scoped error type for Rust applications that values simplicity, semantic clarity, and a stable machine-readable format over stack traces and error chains.

```toml
[dependencies]
mini-err = "0.3"

# Optional: serde support
mini-err = { version = "0.3", features = ["serde"] }
```

---

## Philosophy

Most Rust error crates try to solve *every* problem — dynamic downcasting, backtraces, error chains, span traces, compatibility layers. Mini-err does the opposite: it provides exactly one enum with five variants and a stable `Display` contract, then gets out of your way.

### Why not `anyhow` / `thiserror` / `eyre` / `color-eyre`?

Those crates are excellent for library code and developer-facing diagnostics. Mini-err is designed for the **application boundary** — the place where errors leave your process and become someone else's problem (HTTP responses, CLI exit codes, structured logs). At that boundary you need:

- **A fixed set of categories** – not an open universe of ad-hoc errors.
- **HTTP-friendly status codes** – so you can map directly to 400, 404, 500, 502.
- **A stable `Display` format** – so log aggregators and monitoring can parse without a schema.
- **No hidden allocations** – scope strings are `&'static str`, not `String`.
- **No proc macros** – zero compile-time dependencies.

### Design tenets

1. **Every error has exactly five properties.** Variant, scope, message, HTTP code, and (for IO) an `ErrorKind`. Nothing more.
2. **The display format is part of the API.** `{scope}:{kind}: {message}` — if you depend on mini-err, you may parse this format.
3. **Scope is a `&'static str`.** It identifies *where* the error came from (`"db"`, `"api"`, `"fs"`, `"upstream"`), not a backtrace. This is cheap, stable, and serialization-friendly.
4. **No error chaining.** Only `Io` exposes a `.source()`. For everything else, the message is the complete story. If you need chaining, wrap the mini-err in your own application type.

---

## Variants

| Variant | Code | Meaning |
|---------|------|---------|
| `Bad`   | 400  | Client did something wrong (invalid input, bad request) |
| `Gone`  | 404  | Something was not found (record, route, file) |
| `Cfg`   | 500  | Internal misconfiguration (missing env var, bad config file) |
| `Io`    | 500  | I/O operation failed (disk, network socket, etc.) |
| `Net`   | 502  | Upstream service returned an error |

These map directly to HTTP status codes but are not HTTP-specific — they work equally well for CLI tools, background workers, and GUI applications.

---

## Usage

### Constructing errors

```rust
use mini_err::Error;

// Constructors for each variant
let e = Error::bad("api", "missing required field 'name'");
let e = Error::gone("db", "user not found");
let e = Error::cfg("startup", "DATABASE_URL not set");
let e = Error::net("upstream", "payment gateway timed out");

// Io requires a std::io::Error
let io = std::io::Error::new(std::io::ErrorKind::NotFound, "config.toml");
let e = Error::Io { cause: io, scope: "fs", msg: None };
```

### Using `Result`

```rust
use mini_err::{Error, Result, ErrorExt};

// Result is just std::result::Result<T, Error>
fn load_config(path: &str) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| Error::Io {
            cause: e,
            scope: "fs",
            msg: Some(format!("failed to read {path}")),
        })?;
    Ok(content)
}
```

### Enriching errors with `.context()`

The `ErrorExt` trait adds `.context()` to any `Result<T, E>` where `E: Into<Error>`:

```rust
use mini_err::{ErrorExt, Result};

fn load_config(path: &str) -> Result<String> {
    std::fs::read_to_string(path)
        .context("fs", format!("failed to read config at {path}"))
}

fn parse_port(s: &str) -> Result<u16> {
    s.parse::<u16>()
        .context("api", "port must be a number between 0 and 65535")
}
```

`.context()` overwrites both the scope and the message of the underlying error — it does not wrap or nest.

### From conversions

Standard library errors convert automatically:

```rust
use mini_err::Error;

// std::io::Error → Error::Io
let io: std::io::Error = std::io::Error::new(std::io::ErrorKind::NotFound, "file");
let err: Error = io.into();   // Error::Io { scope: "io", .. }

// ParseIntError → Error::Bad
let err: Error = "not_a_number".parse::<i32>().unwrap_err().into();

// Utf8Error / FromUtf8Error → Error::Bad
```

### Reading error properties

```rust
let err = Error::bad("parse", "missing field 'name'");

err.code();                    // 400
err.scope();                   // "parse"
err.kind();                    // "bad"
err.message();                 // "missing field 'name'"
err.to_string();               // "parse:bad: missing field 'name'"

// Io errors expose the inner io::Error via source()
let io = std::io::Error::new(std::io::ErrorKind::NotFound, "file");
let err = Error::Io { cause: io, scope: "fs", msg: None };
assert!(std::error::Error::source(&err).is_some());
```

### Equality

`Error` implements `PartialEq`. For `Io` errors, comparison is by `io::ErrorKind` and `scope` (the inner `io::Error` does not implement `PartialEq`). All other variants compare by `msg` and `scope`.

```rust
let a = Error::bad("api", "msg");
let b = Error::bad("api", "msg");
assert_eq!(a, b);

// IO errors compare by ErrorKind, not full cause
let a = Error::Io {
    cause: std::io::Error::new(std::io::ErrorKind::NotFound, "file a"),
    scope: "fs", msg: None,
};
let b = Error::Io {
    cause: std::io::Error::new(std::io::ErrorKind::NotFound, "file b"),
    scope: "fs", msg: None,
};
assert_eq!(a, b);  // same kind + scope

assert_ne!(Error::bad("x", "msg"), Error::gone("x", "msg"));
```

---

## Serde support (optional)

Enable the `serde` feature for JSON serialization/deserialization.

```toml
mini-err = { version = "0.3", features = ["serde"] }
```

```rust
use mini_err::Error;

let err = Error::bad("parse", "missing field 'name'");
let json = serde_json::to_string(&err).unwrap();
// {"scope":"parse","kind":"bad","message":"missing field 'name'","code":400,"io_kind":""}

let deserialized: Error = serde_json::from_str(&json).unwrap();
assert_eq!(deserialized, err);
```

The serialized JSON object has these fields:

| Field      | Type   | Always present | Notes |
|------------|--------|----------------|-------|
| `scope`    | string | yes            | |
| `kind`     | string | yes            | `io`, `net`, `cfg`, `bad`, `gone` |
| `message`  | string | yes            | |
| `code`     | number | yes            | HTTP status code |
| `io_kind`  | string | yes            | Stable string like `"NotFound"`, empty for non-IO variants |

On deserialization, the `code` field is validated against the variant — a mismatch produces an error.

The deserializer interns all scope strings in a global pool (capped at 1024 entries). Beyond the cap, novel scopes deserialize as `"overflow"`. This bounds per-process memory regardless of input volume.

---

## Comparison

| Feature                      | mini-err | thiserror | anyhow | eyre |
|------------------------------|----------|-----------|--------|------|
| No proc macros               | ✓        |           | ✓      | ✓    |
| Stable Display contract      | ✓        |           |        |      |
| Categorized variants         | ✓        | opt-in    |        |      |
| HTTP status codes            | ✓        |           |        |      |
| `no_std` support             |          |           |        |      |
| Error chaining / backtraces  |          | ✓         | ✓      | ✓    |
| Dynamic error type           |          |           | ✓      | ✓    |

Mini-err does not aim to replace the above crates. Use it at the *edges* of your application where a stable, scoped, categorised error type is more valuable than flexibility.

---

## MSRV

The minimum supported Rust version is **1.70**. Bumping the MSRV is considered a breaking change.
