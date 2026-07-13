# smallstack — Upgrade & Fix Proposals

A full review of the workspace (all seven crates, the demo, and the infra scripts) with
concrete suggestions for closing security loopholes and improving correctness,
performance, and elegance. **Nothing here has been changed** — each item describes the
problem, why it matters, and how I would implement the fix.

Items are graded: [SEC] security / soundness, [BUG] correctness bug, [PERF] performance,
[ELEG] elegance / API design, [OPT] nice-to-have.

## Summary (highest priority first)

| # | Grade | Crate | Issue |
|---|-------|-------|-------|
| S1 | [SEC] | demo / mini-search | `unsafe impl Send/Sync` papering over an unsound `Processor` bound |
| S2 | [SEC] | mini-static | No header-read timeout → slowloris permanently exhausts the connection semaphore |
| S3 | [SEC] | mini-serve | No TLS handshake timeout → same slow-client DoS on the TLS path |
| S4 | [SEC] | mini-serve / mini-static | Ephemeral test binds listen on `0.0.0.0` |
| S5 | [SEC] | mini-serve | Latent reflect-origin-with-credentials CORS branch; debug/release behaviour divergence |
| S6 | [SEC] | mini-serve | Internal error messages echoed verbatim to clients |
| S7 | [SEC] | all servers | `accept()` error → `continue` can busy-spin at 100 % CPU (EMFILE) |
| S8 | [SEC] | mini-search | Non-atomic persistence write corrupts state on crash |
| S9 | [SEC] | demo | Docker runtime runs as root; no healthcheck |
| S10 | [SEC] | mini-static | Traversal rejections answer 403 with a distinctive message (path-probing oracle); no `nosniff` header |
| C1 | [BUG] | mini-search | Re-adding a document leaves stale entries in the exact & numeric indexes; no delete API at all |
| C2 | [BUG] | mini-search | Documents with any text field > 1 KiB cannot be indexed (query bound applied to documents) |
| C3 | [BUG] | mini-search | Query terms are not normalised like indexed tokens (`cast-iron`, `foo!` never match) |
| C4 | [BUG] | mini-search | Quoted phrase queries silently return zero results |
| C5 | [BUG] | mini-serve | Graceful shutdown can hang while the accept loop waits for a semaphore permit |
| C6 | [BUG] | mini-serve | HEAD fallback forces `Content-Length: 0`, breaking clients that probe size via HEAD |
| C7 | [BUG] | mini-static | No HEAD handling, no `Content-Length` on full responses, no directory redirect |
| C8 | [BUG] | mini-unified | Static files fully buffered in memory; stream errors become silently-truncated 200s |
| C9 | [BUG] | mini-log | The 9th log field silently destroys the 8th |
| C10 | [BUG] | demo | Mutex poisoning bricks every subsequent request after one handler panic |
| P1 | [PERF] | mini-serve | Application state deep-cloned and re-`Arc`ed on every request |
| P2 | [PERF] | mini-serve | Router traverses the trie up to 3× per request and clones param maps while backtracking |
| P3 | [PERF] | mini-static | Root directory re-canonicalised (syscalls) on every request |
| P4 | [PERF] | mini-static | File streaming zeroes + double-copies every 64 KiB chunk |
| P5 | [PERF] | mini-search | Postings cloned per lookup; `avg_field_len` is O(N) per scored term |
| E1 | [ELEG] | mini-serve / mini-static | Impossible triple-nested response fallbacks duplicated six times |
| E2 | [ELEG] | both servers | Four near-identical accept/serve loops; unify |
| E3 | [ELEG] | mini-serve | `path_params` round-trips through a synthetic query string and `serde_qs` |
| E4 | [ELEG] | workspace | `[workspace.dependencies]`, publish script robustness, misc cleanups |

---

## [SEC] Security & soundness

### S1. Unsound `unsafe impl Send/Sync` in the demo — fix the root cause in mini-search

`crates/demo/src/main.rs:48`:

```rust
unsafe impl Send for SearchEngine {}
unsafe impl Sync for SearchEngine {}
```

`Engine` is not `Send + Sync` only because `pipeline: Vec<Box<dyn Processor>>`
(`crates/mini-search/src/engine.rs:54`) has no auto-trait bounds — `Processor`
(`crates/mini-search/src/processor.rs:4`) doesn't require them. The demo's unsafe impls
assert thread-safety the compiler can't verify: if any processor ever holds an `Rc`, a
raw pointer, or a non-`Sync` cell, this is undefined behaviour in safe-looking code.
This is exactly the kind of "this can't happen" that STANDARDS.md Part V forbids.

**Fix (one line in mini-search, minus two lines of unsafe in demo):**

```rust
// processor.rs
pub trait Processor: Send + Sync { ... }
```

or, if non-thread-safe processors must stay possible, change the field to
`Vec<Box<dyn Processor + Send + Sync>>`. Then delete both `unsafe impl` lines from the
demo. Since `Engine`'s other fields are all `Send + Sync`, the engine becomes properly
auto-`Send + Sync`. Add a compile-time regression test:

```rust
fn assert_send_sync<T: Send + Sync>() {}
#[test] fn engine_is_send_sync() { assert_send_sync::<Engine>(); }
```

### S2. mini-static: slowloris exhausts the connection limit permanently

`crates/mini-static/src/server.rs:286` builds connections with no timeouts at all:

```rust
let _ = AutoBuilder::new(TokioExecutor::new()).serve_connection(io, svc).await;
```

Each accepted TCP connection holds a semaphore permit (`max_connections`, default 1024)
until the connection ends. A client that opens sockets and never sends a byte holds its
permit forever. 1024 idle sockets — trivially cheap for an attacker — and the server
stops accepting anyone, with no recovery. mini-serve got this right
(`header_read_timeout`, `app.rs:424`); mini-static did not.

**Fix:** mirror mini-serve. Add `header_read_timeout: Duration` (default 30 s) to
`Server` with a `with_header_read_timeout` builder method, and configure the auto
builder in all serve loops:

```rust
let mut builder = AutoBuilder::new(TokioExecutor::new());
builder.http1()
    .timer(hyper_util::rt::TokioTimer::new())
    .header_read_timeout(header_read_timeout);
let _ = builder.serve_connection(io, svc).await;
```

Add a test in `tests/` that opens a raw TCP socket, sends nothing, and asserts the
server closes it within the timeout (and that a subsequent normal request succeeds).

### S3. mini-serve TLS: no handshake timeout

`crates/mini-serve/src/app.rs:453` and `:572`:

```rust
let tls_stream = match acceptor.accept(stream).await { ... };
```

`header_read_timeout` only starts *after* the TLS handshake completes. A client that
opens TCP and stalls mid-handshake (or never starts it) holds its semaphore permit
indefinitely — the same DoS as S2, on the TLS path.

**Fix:** bound the handshake:

```rust
const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

let tls_stream = match tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, acceptor.accept(stream)).await {
    Ok(Ok(s)) => s,
    _ => return, // drops the permit
};
```

### S4. Ephemeral binds listen on all interfaces

`App::bind_ephemeral` (`app.rs:305`), `bind_tls_ephemeral` (`app.rs:376`) and
`Server::run_ephemeral` (`mini-static/src/server.rs:175`) bind `0.0.0.0:0`. These are
documented as test helpers, but every test run exposes a real server (potentially
serving local files) to the LAN, and triggers firewall prompts on some systems.

**Fix:** bind `([127, 0, 0, 1], 0)`. Any caller who genuinely wants an external
ephemeral bind can use `bind()`/`run()` with an explicit address.

### S5. CORS: remove the latent credentialed-wildcard reflection branch

`crates/mini-serve/src/middleware.rs:54`:

```rust
let origin = if self.credentials && self.allow_all_origins {
    req_origin.unwrap_or("*")
} else ...
```

This branch reflects *any* request origin while `allow-credentials: true` is also
emitted — the classic credentialed-CORS bypass that grants every website scripted access
to authenticated responses. Today it is unreachable (the builder forces `credentials =
false` for wildcard configs), but it is a loaded gun: any future constructor, a
`Deserialize` derive, or a builder refactor that misses the interaction re-opens it.
Dead-but-dangerous code should not exist (STANDARDS Rule 9).

Additionally, `CorsConfigBuilder::build` (`middleware.rs:169`) **panics in debug and
silently degrades in release** for the same misconfiguration. Divergent debug/release
behaviour hides the bug exactly where you'd catch it (dev) or surprises you where you
can't (prod).

**Fix:**
1. Delete the reflection branch — the following `allow_origins.contains("*")` branch
   already handles wildcard (uncredentialed) correctly.
2. Make the misconfiguration unrepresentable at build time in *both* profiles:
   change `build()` to `build() -> Result<CorsConfig, CorsConfigError>` and return an
   error for wildcard+credentials. This is a breaking API change; bump minor version
   and update the two call sites in tests. If you don't want `Result`, panic in both
   profiles — but never "warn and keep going" in release only.

### S6. Internal error messages are echoed to clients

The default error path (`app.rs:207`) sends `e.message` verbatim, and
`impl From<mini_err::Error> for ServeError` (`error.rs:26`) forwards messages — including
`Error::Io` messages that default to `cause.to_string()` (`mini-err/src/error.rs:52`),
i.e. raw OS error text, file paths, and whatever the handler put in the message. The demo
does this today: `Error::gone("item", format!("item '{}' not found", ...))` is fine, but
any `?` on an io error in a future handler leaks internals with a 500.

mini-static already solves this correctly with `StaticError::user_message()`.

**Fix:** sanitise by status class in the default error handler — 4xx messages are
authored for clients and pass through; 5xx bodies become a generic message while the
real message goes to the log (via the `log` feature) or `eprintln!`:

```rust
fn default_error_handler() -> ErrorHandler {
    Arc::new(|status, message| {
        let public = if status.is_server_error() { "internal server error" } else { message };
        error_response(status, public)
    })
}
```

Apps that want full control already have `with_error_handler`.

### S7. `accept()` failure busy-loops

All five accept loops do `Err(_) => continue` (`mini-serve/src/app.rs:408`,
`mini-static/src/server.rs:259`, etc.). Most accept errors are transient per-connection
issues, but `EMFILE`/`ENFILE` (fd exhaustion — an attacker-influenceable state) makes
`accept()` fail *immediately and repeatedly*: the loop spins at 100 % CPU, which turns a
fd-exhaustion nuisance into a full CPU DoS.

**Fix:** on accept error, sleep briefly before retrying:

```rust
Err(_) => {
    tokio::time::sleep(Duration::from_millis(100)).await;
    continue;
}
```

(In the shutdown-aware loops, the sleep occurs inside the select arm and delays shutdown
by at most 100 ms — acceptable; or select the sleep against the shutdown future.)

### S8. mini-search persistence is not crash-safe

`Engine::save` (`crates/mini-search/src/persist.rs:119`) writes `state.json` in place
with `std::fs::write`. A crash, power loss, or full disk mid-write leaves a truncated
file; the next `Engine::open` then fails with "corrupt state file" and the entire index
is lost.

**Fix:** write-to-temp-then-rename (atomic on POSIX):

```rust
let tmp = dir.join("state.json.tmp");
std::fs::write(&tmp, &data).map_err(...)?;
std::fs::rename(&tmp, dir.join(STATE_FILE)).map_err(...)?;
```

Optionally fsync the file and directory for full durability. Also worth doing while
in this file: `validate_path` only rejects literal `..` components — a symlinked
`storage_dir` still escapes. Documenting that the caller owns the directory's trust is
fine; silently accepting any path is not. And see P6 for the double-clone in `save`.

### S9. Docker hardening

`crates/demo/Dockerfile` runs the service as root and defines no healthcheck. If the
demo binary is ever compromised (it parses untrusted JSON and serves files), root in
the container maximises blast radius.

**Fix:**

```dockerfile
FROM debian:bookworm-slim AS runtime
RUN useradd --system --no-create-home --shell /usr/sbin/nologin app
COPY --from=builder /app/target/release/demo /usr/local/bin/demo
COPY --from=builder /app/crates/demo/public /app/public
WORKDIR /app
USER app
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=3s \
  CMD ["/usr/local/bin/demo-healthcheck"]  # or curl -f http://localhost:3000/api/health
CMD ["demo"]
```

(`debian-slim` has no curl; either install it in the runtime stage, or better — add a
`--healthcheck` flag to the demo binary that curls itself and exits, keeping the image
minimal.) In `docker-compose.yml`, add `restart: unless-stopped` and consider
`read_only: true` with a tmpfs for `/tmp`.

### S10. mini-static: traversal responses are an oracle; add `nosniff`

`StaticError::Traversal` → `403` + `"path traversal denied"`
(`crates/mini-static/src/error.rs:43`). Distinguishing "blocked" from "not found" tells
a prober exactly when they've hit the guard and invites iteration. Standard practice is
to answer traversal attempts with the same `404 not found` as any missing file, and log
the attempt server-side.

**Fix:** map `Traversal` to `StatusCode::NOT_FOUND` / message `"not found"` in
`status_code()`/`user_message()` (keep the distinct variant internally for logging).
While touching response headers, add `X-Content-Type-Options: nosniff` to all file
responses — the server serves user-supplied directories, and browsers content-sniffing
mislabelled files is a real XSS vector. One header line in each response builder (or in
the unified builder from E1).

### S11 (minor). demo UI: unescaped attribute interpolation

`crates/demo/public/index.html:223`: `data-id="${docId}"` — `docId` is interpolated
into an attribute without `esc()`. All other interpolations are escaped. With seeded
ids (`p1`…`p12`) it's inert, but the pattern is one copy-paste from stored XSS once ids
become user-supplied. Fix: `data-id="${esc(docId)}"`, and escape `stars(rating)`'s
input by coercing to a bounded number first.

---

## [BUG] Correctness & functionality

### C1. mini-search: document updates corrupt the exact/numeric indexes; no delete API

`InvertedIndex::insert_tokens` correctly calls `self.remove(doc_id, field)` first
(`inverted.rs:117`). But `Engine::add_document` (`engine.rs:106`) inserts into
`ExactIndex` and `NumericIndex` **without removing prior entries**, even though both
types already implement `remove()` (`exact.rs:24`, `numeric.rs:34`). Re-adding a
document with a changed `category` or `price` leaves the *old* value's posting in
place: filters then match documents on values they no longer have, permanently.

There is also no `remove_document` on `Engine` at all — documents are immortal.

**Fix:**

```rust
pub fn remove_document(&mut self, collection: &str, doc_id: &str) -> Result<()> {
    let cfgs = self.field_configs.get(collection).ok_or_else(...)?;
    for (field, _) in cfgs {
        if let Some(inv) = self.inverted.get_mut(collection) { inv.remove(doc_id, field); }
        if let Some(num) = self.numeric.get_mut(collection)  { num.remove(field, doc_id); }
        if let Some(ex)  = self.exact.get_mut(collection)    { ex.remove(field, doc_id); }
    }
    self.documents.get_mut(collection).map(|d| d.remove(doc_id));
    Ok(())
}
```

and make `add_document` begin with `self.remove_document(collection, &doc.id)?` when the
id already exists (which also makes the inverted index's internal remove redundant —
delete it, per Rule 9). Regression test: add doc with `category: "a"`, re-add with
`category: "b"`, assert `category:=a` returns nothing.

### C2. mini-search: documents with >1 KiB of text cannot be indexed

`Tokenizer::tokenize` (`tokenizer.rs:30`) rejects input over `MAX_QUERY_BYTES` (1024) —
but it is called from `InvertedIndex::insert` (`inverted.rs:47`) on **document field
text**. Any document whose `Text` field exceeds 1 KiB makes `add_document` return
`invalid_query` ("input exceeds maximum query length" — for a document!). A search
engine that can't index a three-paragraph description is a functionality cliff, and the
error message misdiagnoses it.

**Fix:** separate the bounds in `bounds.rs`:

```rust
pub const MAX_QUERY_BYTES: usize = 1024;
pub const MAX_DOC_FIELD_BYTES: usize = 1_048_576; // 1 MiB, still a fixed upper bound
```

Give `tokenize` an explicit limit parameter (`tokenize(&text, MAX_DOC_FIELD_BYTES)` at
index time, `MAX_QUERY_BYTES` at query time), keeping Rule 3's fixed bounds while making
each bound fit its input. Test: index a 10 KiB description, search a word from its tail.

### C3. mini-search: query terms bypass the tokenizer's normalisation

Indexing strips non-alphanumeric characters (`tokenizer.rs:68`: only alphanumeric and
`'` are kept), but `Query::parse` (`query.rs:172-182`) merely lowercases the raw token.
So the indexed document text `cast-iron` becomes the token `castiron`, while the query
`cast-iron` searches for the term `cast-iron` — zero hits. Same for any query with
punctuation (`foo!`, `usb-c`, `don't` vs `dont` asymmetries).

**Fix:** run each parsed text clause through the same `Tokenizer` used at index time
(the engine owns one: `engine.tokenizer`). `Query::parse` can stay pure by storing the
raw term; `Engine::search` normalises clause terms via `self.tokenizer.tokenize(&clause.term)`
before lookup, expanding one clause into several terms if tokenisation splits it. One
code path for normalisation, applied at both boundaries — this is the "same path
production uses" principle from STANDARDS Part VI applied to the index/query seam.

### C4. mini-search: quoted phrases can never match

`Query::parse` keeps a quoted phrase as one term (`"hello world"` → term
`hello world`), but the index never contains multi-word terms (the tokenizer only emits
phrase tokens when the *document text itself* contains quote characters — which real
content doesn't). Result: every quoted query silently returns zero hits. A feature that
parses but can't ever match is worse than no feature.

**Fix (choose one, in order of preference per minimalism):**
1. **Tokenize the phrase into AND-terms:** treat `"hello world"` as `hello AND world`
   (require all terms in candidates instead of the current OR). Honest approximation,
   ~15 lines.
2. Implement true positional phrase matching (store token positions in postings). More
   work and memory; only if phrase precision is genuinely needed.
3. Remove phrase syntax entirely (Rule 9).

Also fix the related OR/AND blur: multi-term queries currently union candidates
(`engine.rs:205-216`), which combined with BM25 means `wireless headphones` also returns
every merely-`wireless` doc. Standard expectation is AND; intersecting candidate sets per
term is a small change in the same loop.

### C5. mini-serve: graceful shutdown blocked by semaphore wait

In `serve_with_shutdown` (`app.rs:503`) the permit is acquired *inside* the accept
select-arm:

```rust
result = listener.accept(), if !shutdown_initiated => {
    ...
    let permit = match sem.acquire_owned().await { ... };  // ← select is not running here
```

While all permits are taken, this `await` blocks the entire `select!` — the shutdown
branch cannot fire and `join_set` drain cannot progress. Under connection saturation
(exactly when you're likely to be restarting a struggling service), SIGTERM is
ignored indefinitely. Same structure in the TLS variant and in mini-static's
`serve_with_shutdown`.

**Fix:** make permit acquisition part of the select, so shutdown competes fairly:

```rust
tokio::select! {
    permit = semaphore.clone().acquire_owned(), if !shutdown_initiated => {
        let permit = permit.expect("semaphore never closed");
        let (stream, _) = match listener.accept().await { ... }; // still bounded: permit already held
        join_set.spawn(async move { let _permit = permit; ... });
    }
    _ = &mut shutdown_pin, if !shutdown_initiated => { shutdown_initiated = true; }
    ...
}
```

(Acquire-then-accept also fixes the subtle inversion where a connection is accepted
*before* a permit exists.) Add a test: saturate `max_connections=1`, send shutdown,
assert the future completes once the in-flight request finishes.

### C6. mini-serve: HEAD responses claim `Content-Length: 0`

`app.rs:228-233` serves HEAD via the GET handler (good), then **overwrites**
`Content-Length` with `0`. RFC 9110 §9.3.2 says HEAD should return the same
representation metadata as GET — the entire point of HEAD is to learn the size/headers
without the body. Curl `-I`, CDNs, and download managers will all see size 0.

**Fix:** drop the body without touching headers:

```rust
let (parts, _body) = resp.into_parts();
Response::from_parts(parts, BoxBody::new(Empty::new()))
```

(hyper does not send a body for HEAD regardless; the existing `Content-Length` from the
GET handler is exactly what should be transmitted.)

### C7. mini-static: HEAD, Content-Length, and directory redirects

`handle()` (`server.rs:470`) never looks at the request method:

- **HEAD requests stream the entire file body.** hyper suppresses it on the wire for
  HEAD, but the server still does all the disk I/O; and OPTIONS/POST/DELETE all serve
  files too. Fix: match the method early — GET/HEAD proceed (HEAD skips the body and
  keeps headers), everything else gets `405` with `Allow: GET, HEAD`.
- **No `Content-Length` on full 200 responses** (`server.rs:587-598`): the length is
  known (`meta.len()`) but only the range path sets it (`:545`). Streaming without it
  forces chunked encoding — no client progress bars, no proxy caching by size. Fix: add
  `.header("content-length", file_len.to_string())` on the streaming 200 path (skip when
  a `Transform` will change the size).
- **`/dir` serves `/dir/index.html` without redirecting** (`resolve.rs:31-37`), so every
  relative link inside that page resolves against the parent directory and breaks. Fix:
  in `handle()`, when the resolved path is a directory index and the request path lacks a
  trailing `/`, return `301` with `Location: {path}/`.
- Two spec nits worth one line each: multi-range requests currently get `416`
  (`server.rs:422`) — RFC-correct behaviour when you don't support multipart is to
  ignore the Range header and return `200`; and `If-Range` is unsupported, which can
  hand a client mixed old/new content across a file change — cheap to support since
  the etag already exists (serve full 200 when `If-Range` doesn't match).

### C8. mini-unified: static serving buffers whole files and swallows stream errors

`static_handler` (`mini-unified/src/lib.rs:32`) delegates to
`mini_static::handle_request` (`mini-static/src/server.rs:451`), which `.collect()`s the
entire body — every static response is fully buffered in memory (a 2 GB video = 2 GB of
RAM per concurrent request), and a mid-stream read error is turned into an empty-body
**200** (`unwrap_or_else(|_| Bytes::new())` at `:463`) — a silently truncated success.

**Fix:** eliminate the buffering shim. `handle()`'s body error type is `StaticError`;
mini-serve's is `Infallible`. Bridge by mapping stream errors to an early stream end
plus a log line (the status line has already been sent — aborting the connection is the
only honest signal, which `BodyExt::map_err` into a custom body can do), or change
mini-serve's `ResponseBody` to `BoxBody<Bytes, BoxError>` so error-carrying bodies
compose (bigger, better API change — hyper resets the stream on body error, which is the
correct wire behaviour for a failed transfer). Either way, `handle_request` (the
buffer-everything helper) should be demoted to `#[cfg(test)]` or documented as
small-file-only.

### C9. mini-log: the 9th field destroys the 8th

`Entry::field` (`mini-log/src/entry.rs:56`):

```rust
} else {
    self.overflow_count += 1;
    self.fields[7] = Some(("fields_truncated", self.overflow_count.to_string()));
}
```

When a 9th field arrives, the 8th — real data, already accepted — is overwritten by the
truncation marker. The record then lies twice: a field you logged is gone, and
`fields_truncated=1` undercounts (2 fields were lost: the 8th and 9th).

**Fix:** reserve the last slot for the marker from the start, i.e. accept 7 data fields
and put the marker in slot 8 without destroying anything:

```rust
if self.count < 7 {
    self.fields[self.count] = Some((key, sanitize(&val.to_string())));
    self.count += 1;
} else {
    self.overflow_count += 1;
    self.fields[7] = Some(("fields_truncated", self.overflow_count.to_string()));
    self.count = 8;
}
```

(Or bump the array to 9 with slot 8 reserved, keeping 8 usable fields — the
`LoggingMiddleware` + `err()` combination already uses 4+4, so headroom matters.)
Update the existing overflow test to assert the 8th *accepted* field survives.

### C10. demo: mutex poisoning bricks the API after one panic

Every handler does `state.inner.lock().unwrap()` / `state.search.lock().unwrap()`
(`main.rs:144,165,182,254,285,306,318`). If any handler panics while holding the lock
(hyper catches the panic and keeps the process alive), the mutex is poisoned and **every
subsequent request panics on `unwrap()`** — a self-inflicted permanent outage from one
transient bug, and a direct violation of the project's own "no bare `unwrap()` in
production paths" rule.

**Fix:** recover the guard — for this data structure the state is valid even after a
poisoned write:

```rust
let items = state.inner.lock().unwrap_or_else(|e| e.into_inner());
```

wrapped in a tiny `fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T>` helper used by all
handlers. (Poisoning exists to warn about torn invariants; `next_id`/`Vec::push` have
none that survive a panic mid-way, so recovery is sound here.)

### C11 (smaller correctness notes)

- **mini-serve, path decoding** (`app.rs:174`): the router matches raw, percent-encoded
  paths — `GET /api/%69tems` 404s even though `/api/items` exists. Decode each segment
  (percent-decode, reject `%2F`/`%00`) before matching, in `split_path`.
- **mini-serve, `parse_query`** (`app.rs:51`): `+` is not decoded as space, so HTML-form
  queries (`q=hello+world`) search for `hello+world`. Replace `+` with space before
  percent-decoding, matching `application/x-www-form-urlencoded`.
- **mini-serve, 405 without `Allow`** (`app.rs:248`): RFC 9110 requires an `Allow`
  header on 405. The trie node already knows its methods; return them.
- **mini-serve, CORS preflight for unknown routes** (`app.rs:190`): OPTIONS+Origin gets
  a 204 preflight even for paths with no routes, masking 404s from browser devtools.
  Check `router.has_path` first.
- **mini-static, `..` substring check** (`resolve.rs:13`): `decoded.contains("..")`
  403s legitimate names like `jquery..min.js`. Since `canonicalize` + `starts_with` is
  (correctly, per the comment) the authoritative guard, tighten the pre-check to
  path *segments* equal to `..` rather than any substring.
- **mini-static, non-ASCII filenames** (`resolve.rs:59`): the ASCII-only decoder makes
  `é.png` unservable (documented, but a real gap). Safe upgrade that keeps the
  canonicalize guard authoritative: decode with `percent_encoding::percent_decode_str`
  into **bytes**, build the path via `OsStr::from_bytes` (unix), and keep the segment
  check + canonicalize exactly as-is. The guard, not the decoder, remains the security
  boundary — the comment's concern stays satisfied.
- **mini-err, `context()` destroys the original message** (`result.rs:10`): for
  non-Io variants, `*m = msg.into()` replaces rather than wraps. `parse:bad: invalid
  digit` + context `"reading config"` should yield `"reading config: invalid digit"`,
  not lose the cause. Fix: `*m = format!("{}: {}", msg.into(), m)`.
- **mini-search, `MAX_CANDIDATES` is exported but never enforced** (`bounds.rs:3`): a
  fixed upper bound that doesn't bound anything (Rule 3). Either enforce it in
  `Engine::search` when building candidate sets (early-truncate + document the
  approximation) or delete the constant.
- **mini-search, zero-score ordering**: an empty text query returns all docs with score
  0.0 in HashMap iteration order — nondeterministic across runs. Add `.then_with(|a, b|
  a.0.cmp(&b.0))` id tiebreak in the sort for stable results.

---

## [PERF] Performance

### P1. mini-serve clones the application state on every request

`app.rs:175`:

```rust
let state = State::new(S::clone(&self.state));
```

`App` holds `Arc<S>`; `State` wraps `Arc<S>` — yet each request deep-clones `S` out of
the app's Arc and allocates a *new* Arc around the copy. For the demo's `AppState` that's
4 Arc-clone fields per request (cheap-ish), but for any state holding a `HashMap`,
config, or connection pool it's a real per-request allocation storm, and it also breaks
identity (handlers mutating through interior mutability still work only because the
fields are themselves `Arc`s — a `State<Config>` with plain fields would silently see
copies).

**Fix:** share the existing Arc. Add a private constructor and store the app's Arc
directly:

```rust
impl<S> State<S> {
    pub(crate) fn from_arc(s: Arc<S>) -> Self { State(s) }
}
// route():
let state = State::from_arc(self.state.clone());   // refcount bump only
```

No public API change (`State::new` stays for tests/manual construction). This also lets
`S: Clone` be dropped from several bounds eventually.

### P2. Router: three trie walks per request, clone-heavy backtracking

Per GET request, `route()` walks the trie in `has_path` (`app.rs:196`), again in
`match_route` (`:199`), and for HEAD a third time (`:217`). Each walk calls
`split_path`, which allocates a `Vec<String>` (one String per segment), and `find_node`
(`router.rs:138,149`) clones the entire `PathParams` HashMap at *every static child* it
tries, even though static matches don't add params.

**Fix (single traversal, borrow-only matching):**
1. Change `split_path` to return `Vec<&str>` (or iterate `Split<'_, char>` directly).
2. Return richer information from one walk:
   `fn find(&self, path: &str) -> Option<(&Node<S>, PathParams)>` — then method dispatch
   (`node.handlers.get(method)`), 405 detection (`!node.handlers.is_empty()`), the
   `Allow` header (C11), and the HEAD→GET fallback are all lookups on the *same node*,
   no re-walk.
3. In `find_node`, only clone/insert params in the param-child branch (static branches
   pass `params` through by `&mut` and truncate on backtrack: record `params.len()`
   before descending, roll back on failure). Params become
   `HashMap<&'static str, String>` or stay owned — either way stop cloning per static
   probe.

The routing hot path drops from O(segments × children) allocations to near-zero for
static routes.

### P3. mini-static re-canonicalises the root every request

`resolve()` (`resolve.rs:16`) calls `root.canonicalize()` per request even though
`Server::run` already canonicalised the dir at startup (`server.rs:151`) — two wasted
syscalls (plus an allocation) on the hottest path.

**Fix:** change `resolve`'s contract to take an already-canonical root
(`pub fn resolve(root_canon: &Path, request_path: &str)`), documented as such, and keep
one `debug_assert!(root_canon.is_absolute())`. The public `resolve` docs already say the
server canonicalises; make it true in the signature. Callers that use `resolve`
standalone canonicalise once themselves.

### P4. mini-static file streaming: zeroing + double copy per chunk

`ReadStream::poll_next` (`server.rs:60-86`) `resize`s the buffer (memset 64 KiB) and
then `Bytes::copy_from_slice` copies the filled bytes again — every chunk is written to
memory three times (read, zero, copy).

**Fix:** use `BytesMut` and split off filled chunks — no zeroing (with
`chunk_mut`/unsafe-free `resize` once then reuse), no second copy:

```rust
struct ReadStream<R> { reader: R, buf: BytesMut }

// poll_next:
if this.buf.capacity() < FILE_CHUNK_SIZE { this.buf.reserve(FILE_CHUNK_SIZE); }
this.buf.resize(FILE_CHUNK_SIZE, 0);            // first iteration only re-zeroes reused capacity
...
let chunk = this.buf.split_to(n).freeze();       // zero-copy handoff to Bytes
```

(Even keeping the initial `resize`, `split_to(n).freeze()` removes the per-chunk copy —
the biggest win. `tokio_util::io::ReaderStream` does exactly this if adding the dep is
acceptable; per the dependency gate, the 10-line in-house version is fine.)

### P5. mini-search: postings cloned per lookup; O(N) average-length per term

- `InvertedIndex::postings` (`inverted.rs:77`) clones the whole postings vector. It is
  called once per (term, field) for candidate collection (`engine.rs:210`) and *again*
  for scoring (`score.rs:28`) — every posting list is allocated twice per query term.
  Fix: return `Option<&[(String, usize)]>` and iterate borrows; both call sites only
  read.
- `avg_field_len` → `total_field_len` (`inverted.rs:101`) sums *every document's*
  length on every scored term — O(N) per term, O(N × terms) per query. Fix: maintain
  `total_len: HashMap<String, usize>` incrementally in `insert_tokens`/`remove` (+/-
  tokens.len()), making it O(1).
- `score_collection` (`engine.rs:499`) allocates `vec![clause.term.clone()]` per
  clause×field to call a slice API — pass `&clause.term` through a single-term
  `score_text` variant, or change `score_text` to take `&str`.

### P6 (smaller performance notes)

- **mini-serve `extract.rs`** — see E3; the encode-to-querystring-and-reparse round
  trip is also the perf issue.
- **mini-search `persist.rs:109`** — `save()` clones the entire document store and all
  configs before serialising. Define a borrow-carrying `PersistedStateRef<'a>` with
  `&'a HashMap<...>` fields (serde serialises references fine) — halves peak memory of
  a save.
- **mini-log `entry.rs:11`** — `sanitize` returns a fresh `String` even on the clean
  fast path (every field, every log line). Return `Cow<'_, str>` and only allocate when
  dirty; `render` (`format.rs:22`) should use `write!(out, ...)` into the existing
  String instead of `push_str(&format!(...))`.
- **Docker build caching** — the Dockerfile copies all sources before `cargo build`, so
  any source edit rebuilds all dependencies. Standard fix: `cargo chef` (or the
  dummy-`main.rs` trick) to cache the dependency layer; cuts warm image builds from
  minutes to seconds.

---

## [ELEG] Elegance & API design

### E1. Extract the "impossible fallback" response helper

The triple-nested `unwrap_or_else` fallback (build response → fallback 500 → fallback
500 again → `unwrap()`) appears six times: `mini-serve/src/response.rs:51,70,93` and
`mini-static/src/server.rs:599,639,672`. Each is ~15 lines of ceremony for a case that
is *statically impossible* — a `Response::builder()` with only a status and
pre-validated static headers cannot fail. ~90 lines → ~10:

```rust
/// Build a response; on (impossible) builder failure, fall back to a
/// header-free 500, which cannot fail: status-only builders are infallible.
fn respond_or_500(builder: hyper::http::response::Builder, body: ResponseBody) -> Response<ResponseBody> {
    builder.body(body).unwrap_or_else(|_| {
        let mut r = Response::new(empty_body());
        *r.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        r
    })
}
```

`Response::new` + `status_mut` involves no fallible builder, so no nesting is needed —
the current inner fallbacks guard against a failure mode that cannot occur.

### E2. Unify the four accept/serve loops

`mini-serve` has `serve_inner`, `serve_tls_inner`, `serve_with_shutdown`,
`serve_tls_with_shutdown` (~200 lines, 90 % identical); `mini-static` repeats the
pattern twice more, plus the `LogFn` construction copy-pasted three times
(`server.rs:158,190,219`). Any fix (S2, S7, C5) currently needs to be applied in six
places — which is exactly how the header-timeout divergence (S2) happened.

**Fix:** one generic loop per crate:

```rust
async fn serve_loop<S, F>(
    listener: TcpListener,
    app: Arc<App<S>>,
    tls: Option<TlsAcceptor>,          // cfg(feature = "tls") — or a small enum
    shutdown: Option<F>,               // None = run forever
) { ... }
```

The non-shutdown variants become `serve_loop(l, app, tls, None::<Never>)`; a
`fn make_log(logger) -> Option<LogFn>` helper kills the triplicated closure. Net
deletion of ~250 lines across the two crates (Rule 9), and future security fixes land
in one place.

Two more in the same spirit:
- `route()`'s HEAD fallback (`app.rs:215-246`) duplicates the entire dispatch block;
  extract `async fn dispatch(handler, req, query, params, max_body) -> Response` used by
  both arms (and by P2's single-traversal refactor).
- `Node`'s `segment`/`param_name`/`is_wildcard` tri-state (`router.rs:20`) invites
  invalid states; replace with `enum Seg { Static(String), Param(String), Wildcard }` —
  `insert`'s three near-identical find-or-create branches collapse into one keyed on
  `Seg`.

### E3. Replace the `path_params` querystring round-trip

`extract.rs:22-39` deserialises path params by URL-encoding the HashMap into a
synthetic query string (with per-byte `format!` allocation in `url_encode`) and parsing
it back with `serde_qs`. That's two encoders, an extra dependency, and sorted-pairs
subtlety to express "deserialise a `HashMap<String, String>` into T".

**Fix:** serde's built-in map deserialiser does it directly:

```rust
use serde::de::value::{MapDeserializer, Error as DeError};

pub fn path_params<T: DeserializeOwned, B>(req: &Request<B>) -> Result<T, ServeError> {
    let params = req.extensions().get::<PathParams>()
        .ok_or_else(|| ServeError::new(500, "no path params in request extensions"))?;
    T::deserialize(MapDeserializer::<_, DeError>::new(
        params.0.iter().map(|(k, v)| (k.as_str(), v.as_str())),
    ))
    .map_err(|_| ServeError::new(400, "invalid path parameters"))
}
```

Caveat: `MapDeserializer` feeds values as strings, so numeric fields like the demo's
`id: u64` need `deserialize_str` → parse; wrap values in a tiny `StrValueDeserializer`
(~20 lines implementing forwarding `deserialize_*` that parse from the &str) — still a
net win: `serde_qs` and `url_encode` are deleted, and mini-serve's dependency count
drops by one toward the project's own ≤5 gate. Add a matching **`query_params<T>`**
extractor over `QueryParams` for API symmetry — today path params get typed extraction
but query strings are stringly-typed by hand (see `search_handler`).

### E4. Workspace & tooling cleanups

- **`[workspace.dependencies]`**: hyper/tokio/serde/serde_json/http-body-util versions
  are repeated across six manifests. Centralise in the root `Cargo.toml` and use
  `dep.workspace = true` — one place to bump, no accidental version skew. Also:
  `workspace.package.version = "0.1.0"` is inherited by nothing (every crate sets its
  own) — delete it.
- **`publish.sh`**: (a) if `cargo publish` fails, `set -e` aborts *before*
  `mv "$backup" "$manifest"` — the working tree is left with a mangled manifest. Add
  `trap 'restore_manifests' EXIT`. (b) The whole sed-rewrite exists because
  `mini-unified` and `demo` declare path deps without `version =`; adding
  `version = "…"` there (as the other crates already do) plus a committed
  `registry = "mini"` key lets stock `cargo publish` strip the `path` itself, and the
  script shrinks to a version-check loop.
- **`Engine`'s duplicated `Debug` impl** (`engine.rs:57-85`): the two cfg branches
  differ by one field. Use the non-consuming builder API:
  ```rust
  let mut d = f.debug_struct("Engine");
  d.field("documents", &self.documents) /* … */;
  #[cfg(feature = "persist")] d.field("storage_dir", &self.storage_dir);
  d.finish()
  ```
- **`read_classification`** (`mini-static/src/server.rs:49`): classifying an io result
  into `"eof" | "chunk" | "error"` strings and re-matching with a `_ => unreachable!()`
  arm trades type safety for stringliness. Match directly on `(result, n)` — the enum
  and the unreachable arm both disappear.
- **SSE frame duplication**: `mini-unified`'s `ReloadStream` (`lib.rs:99-123`)
  re-implements `mini-static/src/live.rs`'s `SseStream` byte-for-byte. Export one
  `fn reload_event_frame(&ReloadEvent) -> Bytes` from mini-static and share it.
- **`GroupBuilder` / `RouteBuilder` method duplication** (`app.rs:792-810` vs
  `:889-907`): four identical verb methods each. A small macro
  (`route_verbs!(get => GET, post => POST, …)`) or a shared `trait Routes` keeps them in
  lockstep when (e.g.) `patch()` is added.
- **`sse_stream`'s `Sync` bound** (`response.rs:87`): `S: … + Sync` is unnecessary for
  a body that is polled by one task; it needlessly rejects valid streams. Drop it.
- **mini-log `Logger.scope: &'static str`** forces compile-time scopes; fine for the
  design, but then `mini_err::ser::intern_scope`'s leak-based interner exists only to
  satisfy it on deserialisation. Consider `Cow<'static, str>` for both — deletes the
  interner (and its `MAX_SCOPES` bound) outright.

---

## [OPT] Functional upgrades worth considering (kept minimal, per the manifesto)

1. **mini-static: cache-policy knob.** Every response is `cache-control: no-cache`
   (revalidate-always). Fingerprinted assets deserve
   `public, max-age=31536000, immutable`. One builder method:
   `with_cache_control(fn(&Path) -> &'static str)` — zero-config default stays
   `no-cache`.
2. **mini-static: precompressed sidecar support.** If `foo.js.gz`/`.br` exists and the
   client sends `Accept-Encoding`, serve it with `Content-Encoding` + `Vary`. ~40 lines,
   no compression dependency, huge bandwidth win for the static-site use case.
3. **mini-serve: optional per-request deadline.** `header_read_timeout` bounds the
   headers but a handler + slow body can hold a connection forever
   (`with_request_timeout(Duration)` wrapping `route()` in `tokio::time::timeout`,
   returning 503/408 on expiry). Complements S2/S3 as the third leg of connection
   lifecycle hygiene.
4. **mini-search: expose `search` with a limit parameter.** `MAX_RESULTS = 100` is a
   good ceiling, but callers can't ask for the top 10 without receiving and discarding
   100 redacted document clones (`truncate(limit.min(MAX_RESULTS))`).
5. **Demo: `/api/search/seed` is an unauthenticated state-reset POST.** Fine for a demo;
   worth a comment in `main.rs` so it never survives a copy-paste into a real service.

## Suggested execution order

Per the project's own triage rule (MVP → DX → Performance → Features):

1. **Soundness/security first, they're small:** S1, S3, S4, S7, S9, S11 are each ≤10
   lines. S2 (timeout plumbing), S5 (CORS builder Result), S6, S8, S10 are an
   afternoon together.
2. **Correctness cluster in mini-search** (C1–C4 + tiebreak) — these change scoring/
   matching behaviour, so land together with their regression tests in one minor bump.
3. **Serve-loop refactor as the enabler:** E2 first, then C5/S2/S7 land once instead of
   six times; C6/C7 ride along.
4. **Performance batch** (P1–P5) after the refactors, each with a micro-benchmark
   (`criterion` dev-dep or a `#[bench]`-style test) so the wins are measured, not
   vibes — as STANDARDS Part IX demands.
5. **E3/E4 and the [OPT] items** as opportunistic follow-ups.
