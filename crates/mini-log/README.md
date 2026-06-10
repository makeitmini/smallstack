# mini-log

A minimal, structured logger for Rust applications that values scoped output, field-level sanitation, and a choice between human-readable and JSON format — with zero proc macros and a single dependency (`serde_json`).

```toml
[dependencies]
mini-log = "0.3"

# Optional: mini-err integration
mini-log = { version = "0.3", features = ["err"] }
```

---

## Philosophy

Most Rust logging crates either target the `log` facade (which forces string formatting at every callsite) or bring in async runtimes, span trees, and dynamic dispatch. Mini-log does neither. It is a *logger*, not a *logging framework* — you create a `Logger`, call methods on it, and get output.

### Why not `log` + `env_logger` / `tracing` / `slog` / `flexi_logger`?

Those crates are excellent for large applications with complex observability needs. Mini-log is designed for the **middle ground** — programs that need structured fields, level-based filtering, and JSON output, but don't want to buy into the `log` crate's macro infrastructure or `tracing`'s span model.

- **No macros.** `log.info("msg").field("key", val).emit()` — plain method calls.
- **No formatting at callsites.** Messages are `&'static str`; fields are attached via `.field()`.
- **No string interpolation injection.** Every field value is sanitized — newlines, carriage returns, null bytes, and ANSI escape codes are stripped (tabs are preserved).
- **Predictable allocation.** Up to 8 fields per entry, stored in a fixed-size array. Overflow is tracked, not silently dropped.
- **Shared writers.** Cloning a `Logger` shares the same output handle (via `Arc<Mutex<…>>`), so you can pass loggers to threads without coordinating ownership.

### Design tenets

1. **A logger is a value.** It has a scope, a level filter, a format, and a writer. Pass it around, clone it, configure it.
2. **Messages are `&'static str`.** They describe *what happened*. Dynamic data goes into fields.
3. **Fields are opt-in.** No key-value pairs unless you add them. No structured context unless you ask for it.
4. **Log injection is not the caller's problem.** All field values are sanitized at the boundary. The logger guarantees safe output even if a field comes from untrusted input.
5. **Two output formats, stable contracts.** Both the conventional and JSON formats are part of the API. Downstream code may parse them.

---

## Usage

### Creating a logger

```rust
use mini_log::{Logger, Level, Format};

// Default: stdout, Info level, Conventional format
let log = Logger::new("my-app");

// Configured builder
let log = Logger::new("api")
    .with_level(Level::Debug)
    .with_format(Format::Json);
```

### Logging messages

```rust
let log = Logger::new("http");

log.info("request_started").emit();
log.warn("slow_query").field("duration", "430ms").emit();
log.error("connection_failed").emit();
```

Each level method returns an `Entry` that must be `.emit()`-ed. The entry is discarded if the logger's level filter rejects it — so it is safe to build expensive field values after the level check (they are never constructed).

### Structured fields

Attach key-value pairs with `.field()`:

```rust
log.info("user_created")
    .field("user_id", 42)
    .field("role", "admin")
    .emit();

// Conventional output:
// [1741712345.678] info(http): user_created user_id=42 role=admin
//
// JSON output:
// {"level":"info","scope":"http","msg":"user_created","user_id":"42","role":"admin","ts":1741712345678}
```

Field values are converted via `Display` and sanitized (control characters and ANSI escapes are replaced, tabs preserved).

Up to **8 fields** per entry. The 9th field replaces the last slot with `fields_truncated=N`:

```rust
log.info("many")
    .field("a", 1).field("b", 2).field("c", 3).field("d", 4)
    .field("e", 5).field("f", 6).field("g", 7).field("h", 8)
    .field("i", 9)  // overflow → fields_truncated=1
    .emit();
```

### Duration tracking

```rust
let start = std::time::Instant::now();
// … work …
log.info("operation_complete").duration(start).emit();
// Adds: duration=42ms
```

### Level filtering

`Level` implements `PartialOrd` — entries below the logger's threshold are silently dropped:

```rust
use mini_log::Level;

let log = Logger::new("app").with_level(Level::Warn);

log.error("disk_full").emit();   // output
log.warn("high_memory").emit();  // output
log.info("request_ok").emit();   // suppressed
log.debug("x = 1").emit();      // suppressed
log.trace("entered loop").emit();// suppressed
```

Level ordering: `Error < Warn < Info < Debug < Trace`.

### Environment-driven configuration

```rust
let log = Logger::from_env("my-app");
```

Reads `LOG_LEVEL` and `LOG_FORMAT` from the environment:

| Env var      | Values                                | Default |
|--------------|---------------------------------------|---------|
| `LOG_LEVEL`  | `error`, `warn`, `info`, `debug`, `trace` | `info`      |
| `LOG_FORMAT` | `conventional`, `json`                | `conventional` |

Values are case-insensitive. Unrecognized values fall back to the default.

### Custom writers

Replace stdout with any `Write + Send + Sync`:

```rust
use std::sync::{Arc, Mutex};

let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
let writer: Arc<Mutex<Box<dyn Write + Send + Sync>>> = Arc::new(Mutex::new(
    Box::new(std::fs::File::create("app.log").unwrap()),
));
let log = Logger::new("app").with_writer(writer);
```

### Cloning

Cloning a `Logger` shares the same writer handle, level, format, and scope:

```rust
let log_a = Logger::new("app");
let log_b = log_a.clone();

log_a.info("from_a").emit();
log_b.info("from_b").emit();
// Both entries go to the same output.
```

---

## Output formats

### Conventional

```
[{unix_secs}.{millis:03}] {level}({scope}): {msg} {key}={val} …
```

```
[1741712345.678] info(http): request_started
[1741712345.789] warn(http): slow_query duration=430ms
[1741712346.001] error(http): connection_failed
```

The timestamp is a Unix epoch timestamp with millisecond precision (two integer fields separated by `.`).

### JSON

```json
{"level":"info","scope":"http","msg":"request_started","ts":1741712345678}
{"level":"warn","scope":"http","msg":"slow_query","duration":"430ms","ts":1741712345789}
```

Every JSON entry contains `level`, `scope`, `msg`, and `ts` (Unix millisecond timestamp). All fields are strings. The output is one object per line.

---

## Mini-err integration (optional)

Enable the `err` feature for `.err(&mini_err::Error)` which maps an error's properties into four fields:

```toml
mini-log = { version = "0.3", features = ["err"] }
```

```rust
use mini_log::Logger;
use mini_err::Error;

let log = Logger::new("http");
let err = Error::bad("parse", "missing field 'name'");

log.error("request_failed").err(&err).emit();

// Adds: err_scope=parse err_kind=bad err_msg=missing field 'name' err_code=400
```

This is a convenience over calling `.field()` four times manually. It works with both Conventional and JSON output.

---

## Sanitization

Every field value passed to `.field()` is sanitized before output:

| Character(s)           | Treatment       |
|------------------------|-----------------|
| `\t` (tab)             | Preserved       |
| `\r`, `\n`, `\0`       | Replaced with space |
| Bytes `0x01`–`0x1F` (excl. tab) | Replaced with space |
| `0x7F` (DEL)           | Replaced with space |
| `0x1B` (ANSI escape)   | Replaced with space |

This prevents log-injection attacks and terminal-escape-sequence injection without placing the burden on each callsite.

---

## Comparison

| Feature                         | mini-log | log + env_logger | tracing | slog |
|--------------------------------|----------|------------------|---------|------|
| No proc macros                 | ✓        | ✓                |         |      |
| Single non-optional dependency | 1        | many             | many    | many |
| Field sanitization             | ✓        |                  |         |      |
| Structured fields              | ✓        |                  | ✓       | ✓    |
| JSON output                    | ✓        |                  | ✓       | ✓    |
| `log` crate compatibility      |          | ✓                | ✓       | ✓    |
| Async tracing / spans          |          |                  | ✓       |      |

Mini-log is best suited for **applications that want structured, safe logging without buying into a macro ecosystem or async runtime**. If you need span trees, async tracing, or compatibility with the `log` facade, consider `tracing`. If you want zero-config unstructured logging, `env_logger` is simpler. Mini-log fits in the middle.

---

## MSRV

The minimum supported Rust version is **1.70**. Bumping the MSRV is considered a breaking change.
