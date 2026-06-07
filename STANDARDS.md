# smallstack — Standards

---

## Pre-commit Checklist

Applies to every commit:

- [ ] Primary integration test was written first, was failing, now passes
- [ ] Expected values calculated from spec, not from a prior run
- [ ] Every new error variant has a test asserting the exact variant
- [ ] No `.unwrap()` in non-test production paths
- [ ] No `#[allow(unused)]` or `todo!()` in committed code
- [ ] `cargo test --workspace` passes with zero warnings
- [ ] Every public item has a `///` doc comment
- [ ] Every comment still accurately describes the code next to it
- [ ] All expected values in assertions were derived from spec, not from a prior run

---

## Coupling Rules

Each crate is independently publishable. Optional sibling deps are expressed
through Cargo feature flags only:

| Crate | Standalone deps | Optional sibling deps |
|---|---|---|
| `mini-err` | `serde` (optional) | none |
| `mini-log` | `serde_json` | `mini-err` behind `"err"` |
| `mini-serve` | `hyper`, `hyper-util`, `http-body-util`, `http-body`, `tokio`, `serde`, `serde_json`, `serde_qs`, `futures-core` | `mini-err` behind `"err"`, `mini-log` behind `"log"` |
| `mini-static` | `hyper`, `hyper-util`, `http-body-util`, `http-body`, `tokio`, `serde_json`, `mime_guess` | `mini-err` behind `"err"`, `mini-log` behind `"log"` |

---

## Sibling Dependency Pattern

Optional sibling deps use both `path` and `version` in Cargo.toml:

```toml
mini-err = { path = "../mini-err", version = "0.3", optional = true }
```

- `path` directs cargo to the local crate during development.
- `version` is what ships to crates.io — `cargo publish` strips `path`.

**Version drift risk**: `version = "0.3"` means `>=0.3.0, <0.4.0`. If `mini-err`
bumps to `0.4.0`, the dep won't pick it up. Locally `path` overrides this, so
`cargo test` will pass while the published constraint is too narrow.

**Pre-publish check**: Before publishing a crate with sibling deps, run:

```
cargo publish -p <crate> --dry-run
```

This validates that the version constraints resolve against crates.io. CI
should include this check for any crate that has sibling dep entries.

---

## Commenting Standards

1. **Comments must stay accurate** — update or remove stale comments in the
   same commit that changes the code.
2. **Comments explain *why*, not *what*** — the code already says what it does.
3. **No commented-out code** — that is what version control is for.
4. **`///` doc comments on all public API items** — they are part of the
   interface contract.
5. **No `// TODO:` without an issue reference.**

---

## Method Discipline

### Testing

- All tests live in `tests/` as integration tests. No test code in `src/`.
- Exception: pure-logic modules with no public API surface (e.g. `router.rs`'s trie)
  may keep inline `#[cfg(test)]` unit tests for internal implementation coverage.
  Every such exception must be justified by a comment at the module level.
- Every test follows Arrange / Act / Assert explicitly.
- Test names describe the scenario and the expected outcome, not the function
  under test: `stale_lamport_is_rejected` is correct; `test_append_event` is not.
- Every error variant has at least one test asserting the exact error type.
- No test asserts `.is_ok()` or `.is_err()` alone — follow with an assertion
  on the effect.

### Code Style

- Functions are verbs, types are nouns, booleans are assertions.
- One primary concern per file. Implementation in `src/`, tests in `tests/`.
- No circular dependencies between modules.
- Public API surface is the minimal set of types and functions a consumer
  actually needs.
- Before adding any external dependency: does std solve it? Can you implement
  the 20% you need in less time than evaluating the dependency?

### Architecture

- **MVP first** — ship the minimal working version, then iterate.
- **Interfaces over implementation** — depend on abstractions at every boundary
  that matters.
- **Explicit over implicit** — dependencies, assumptions, and side effects must
  be visible at the call site.
- **Composition over inheritance** — small focused units connected at explicit
  seams.
