# The Smallstack Standard

> A unified coding manifesto, style guide, and set of best practices.
> **Unix clarity + SOLID architecture + NASA Power of 10 reliability.**

This document is the single source of truth for how we build. It consolidates the
method, the testing standard, and the commenting standard into one place, and adds a
short section of recommended practices that extend the same philosophy. It is written
to be language-agnostic; the examples are illustrative, the principles are not.

-----

## Part I — Philosophy

We build tools that are **minimally viable but rock solid**. The two halves are equally
load-bearing. Minimal without solid is a toy; solid without minimal is the enterprise
bloat we are reacting against.

Four values sit underneath everything:

- **Minimally viable.** Each unit does one thing well. No feature bloat. The smallest
  thing that fully solves the problem beats the largest thing that mostly solves it.
- **Rock solid.** Production-grade reliability and security are built in from the first
  commit, not bolted on before launch.
- **Zero config.** Sensible defaults, batteries included. The happy path requires no
  decisions; every decision that remains is one the user genuinely needs to make.
- **Escape-hatch friendly.** No lock-in. Any component can be ripped out and replaced
  because it depends on interfaces, not on its neighbours’ internals.

The animating test for all of it: **does it fit in your head?** Code you can hold in your
head is code you can reason about, test, and delete. Everything below is in service of
keeping the system small enough to understand and strict enough to trust.

-----

## Part II — The Ten Rules

These are the spine. Everything in Parts III–VIII elaborates on one of them.

1. **One responsibility.** Every module, file, and function does exactly one thing. If
   you can’t describe it in a single sentence, split it.
1. **Bounded complexity.** Functions ≤ 60 lines. Modules ≤ 1000 lines. An MVP ≤ 500 lines
   total. If it doesn’t fit in your head, it’s too complex.
1. **Fixed upper bounds.** No unbounded loops, recursion, queues, buffers, or retries.
   Every loop has a provable exit.
1. **Minimal dependencies.** Every dependency is a liability. Prefer the standard library.
   If you need 20% of a library, implement that 20%. Production code: ≤ 5 dependencies.
1. **Explicit over implicit.** No hidden state, no magic configuration, no ambient
   globals. Every dependency is passed in. Every assumption is visible at the call site.
1. **Errors are values.** No ignored errors. No bare panics, `unwrap()`, or unchecked
   nulls in production paths. Every error variant has an explicit handler and a test
   asserting the exact variant.
1. **Interfaces over implementations.** Depend on traits/interfaces/protocols, not
   concrete types. Code is only as coupled as its interface requires.
1. **Tests disprove, not describe.** Every commit’s primary test crosses the boundary the
   commit claims to connect. Expected values are calculated from the spec, never copied
   from a prior run.
1. **Deletion is progress.** Removing code is a feature. Features compose externally;
   nothing is added to the core. Before adding, ask what can be removed.
1. **Ship the method, not the vision.** MVP → DX → Performance → Features, in that order.
   One boundary per commit. No commit ships without a failing test written first.

-----

## Part III — Architecture & Design

**Single responsibility is the cut, not just the label.** The one-sentence test (Rule 1)
is a real gate. “Handles users” is not a responsibility — it’s a department. “Validates a
user-creation request” is. When the sentence needs an “and,” you have found a seam; cut
there.

**Complexity bounds are hard limits, not aspirations.** The numbers in Rule 2 exist so
that “too big” is an objective fact rather than a judgment call you can argue yourself out
of at 2am. A function pushing 60 lines is telling you it has more than one job.

**Make the implicit explicit.** Pass dependencies in; don’t reach out for them. A function
that reads a global, a singleton, or the ambient clock is lying about its inputs — its
signature claims one thing and its behaviour depends on another. Explicit dependencies are
what make the escape hatches real: you can only swap what you can see being passed.

**Depend on interfaces.** Concrete types couple you to decisions you haven’t finished
making yet. A trait/interface is a contract narrow enough to honour and broad enough to
re-implement. This is what lets a component be replaced “as you grow” without a rewrite.

**Deletion is the default move.** The codebase that stays comprehensible is the one where
removal is celebrated. New behaviour belongs *beside* the core, composed in — not *inside*
it. Before every addition, the honest question is: what can I take out instead?

-----

## Part IV — Dependencies

Before anything enters the stack, it must pass all four gates. If any answer is
unsatisfying, implement the slice you need instead.

|Gate            |Question                                                                                                  |
|----------------|----------------------------------------------------------------------------------------------------------|
|**One sentence**|What does it do? If you can’t say it in one sentence, you don’t understand it well enough to depend on it.|
|**≤ 5 deps**    |What does it pull in transitively? A small library with a large tail is a large library.                  |
|**5-year test** |Could you maintain this yourself if upstream went dark tomorrow?                                          |
|**Composable**  |Does it enhance the system without coupling to it?                                                        |

The bias is always toward owning a small, understood slice over importing a large,
unknown one. A dependency is borrowed complexity that comes due at the worst possible
time — during an incident, on someone else’s schedule.

-----

## Part V — Error Handling

Errors are values, and values get handled — at every call site, every time.

- **No silent failures.** An ignored error is a bug you’ve agreed to be surprised by later.
- **No bare panics in production paths.** `unwrap()`, unchecked nulls, and “this can’t
  happen” are how production learns it can happen. Handle the variant or propagate it
  deliberately.
- **Errors are typed and specific.** A generic “something went wrong” is unactionable. Each
  failure mode is a distinct variant the caller can branch on.
- **Every variant has a test.** Rule 6 isn’t satisfied by handling the error — it’s
  satisfied by a test that asserts the *exact* variant comes back from the *exact*
  condition. See Part VI.

-----

## Part VI — Testing

This is the longest section because it is the one most often faked. A green suite is only
meaningful if the tests were written to **disprove** the implementation, not to
**describe** it.

### The core rule

**Every commit’s primary test crosses the boundary the commit claims to connect.**

If a commit wires module A into module B, its primary test calls into both A and B
together, through the same path production uses. A test that exercises only A, or only B,
does not verify the commit — no matter how green it is. And if that integration test
*can’t* be written yet because a dependency is missing, the commit isn’t ready. Reorder
the work.

### What makes a test genuine

A test earns its place only if all of these hold:

1. **It can fail.** If no possible bug makes it red, it’s a comment with ceremony.
1. **It tests behaviour, not structure.** Assert on observable outcomes — a return value, a
   state change, an error — never on naming, internal shape, or “this function was called.”
1. **It specifies the expected value explicitly.** `assert_eq!(price, 120)` beats
   `assert!(result.is_ok())`. The expected value is computed independently — by hand, from
   the spec, or from a known-good reference.
1. **It would catch a regression.** If a plausible one-line change broke the behaviour,
   this test goes red. If it wouldn’t, it isn’t coverage.
1. **It verifies the commit’s stated purpose end-to-end.** Edge-case and error tests are
   additive, not substitutes for the one test that proves the commit did its job.

### The Potemkin catalogue — tests that lie

|Anti-pattern                                                                                                |Why it’s worthless                                                           |
|------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------|
|**The tautology** — folding an empty list and asserting nothing changed                                     |Mathematically guaranteed to pass; proves nothing                            |
|**Isolation passed off as integration** — testing unit A alone while claiming you wired A into B            |Doesn’t touch the boundary the commit exists to create                       |
|**The structural assertion** — `assert!(json.contains("Active"))`                                           |Passes even on malformed output; round-trip and assert value equality instead|
|**The existence check** — `assert!(op.is_ok())` and nothing more                                            |Confirms it didn’t explode, not that it did the right thing                  |
|**The copied expected value** — running broken code, seeing `101`, asserting `101`                          |Encodes the bug as the spec                                                  |
|**The mirror** — test so structurally coupled to the implementation that a wrong implementation still passes|Write the test from the spec *first*, then make it pass                      |

The fix for every row is the same move: assert the **exact value the spec requires**,
through the **real boundary**, with the expected value derived **independently** of the
code under test.

### Structure

- **Tests live outside the implementation.** Use an integration-test layout (`tests/`,
  separate test target, etc.) so tests consume the public API exactly as real callers do
  and can’t reach private internals.
- **One commit, one primary test.** Written before the implementation. Red on the current
  tree. The commit is done when it goes green — not before.
- **Arrange / Act / Assert, strictly.** No assertions in arrange. No setup in assert.
- **Name the scenario and outcome, not the function.** `stale_lamport_is_rejected` and
  `bid_price_reflects_configured_increment`, not `test_append_event`.
- **Error-path tests assert the exact variant.** `assert!(matches!(err, DomainError::StreamNotFound(_)))`, not `assert!(res.is_err())`.
- **Helpers produce minimal valid values; tests override only what matters.** Name helpers
  for what they produce, not how. Use non-default-looking numbers in fixtures
  (`price_increment: 25`, not `1`) so a test that grabs the wrong field still fails — `+1`
  is exactly the result a hardcoded bug produces.

### Boundary tests are the important ones

For every seam where one module hands off to another, there is a required test that drives
the real handoff and asserts the real result: store → manager, manager → state derivation,
state → rules, commit → broadcast → subscriber, and so on. The pattern is invariant: run
two inputs that the spec says should diverge, and prove they diverge through the actual
wiring. A boundary test that two different configs produce two different outputs cannot
pass if the boundary isn’t really connected.

### The question to ask before committing

> If I introduced a plausible one-line bug into this commit’s implementation, would at
> least one test catch it?

If the answer is no for any part of the commit’s stated purpose, the suite is incomplete.
A passing suite that can’t catch a plausible regression is not a passing suite — it’s a
green light that means nothing.

-----

## Part VII — Comments & Documentation

Comments cause concrete harm in two specific ways: they lie about what code does, and they
linger as dead code that version control should be holding instead. The rules address only
those harms. Volume and placement are matters of taste and are left to you.

### 1. Comments must stay accurate

A stale comment is a correctness bug, not a style nit — it costs the next reader time and
confidence. If you change code a comment describes, update or delete the comment **in the
same commit**. Owning a code change means owning its comments.

### 2. Comments explain *why*, not *what*

The code already says what it does. A comment that restates it in English is pure
maintenance burden. A comment that explains the non-obvious *why* — the constraint, the
gotcha, the reason this isn’t the naive version — is the thing that would otherwise cost a
trip to `git blame` or a wrong assumption.

```
// BAD — restates the code
// increment the price by the configured increment
price += config.increment;

// GOOD — explains the non-obvious constraint
// strictly-greater, not >=: an equal timestamp is a replay, not a new event
if lamport <= last_committed { return Err(StaleLamport); }
```

If a *what*-comment genuinely helps the reader, write it — but first notice whether it’s
covering for code that should simply be clearer, and fix the code instead.

### 3. No commented-out code in committed files

It accumulates, it confuses, and it is never coming back. If you might need it, that’s what
branches and tags are for. If it’s work-in-progress, that’s what a stash is for. The only
exception is a `// TODO:` tied to a tracked issue with a real horizon — a floating
`// TODO: fix this` is commented-out intent; remove it or file the issue.

### Doc comments

Public-API doc comments are part of the interface contract and follow the same accuracy
rule: if behaviour changes, the doc comment changes. Write them where they help consumers;
skip them on private helpers where they’d only restate the obvious to the implementer.

-----

## Part VIII — The Commit Discipline

The unit of progress is one boundary, crossed, with the test that proves it. Both
overlapping checklists in the source material collapse into this single gate.

**One boundary per commit.** A commit connects exactly one seam. If your diff touches two
boundaries, it’s two commits.

**Test-first, always.** Write the primary integration test for the boundary before the
implementation. Watch it fail. Then make it pass. A commit with no failing-test-first did
not follow the method, regardless of how it looks.

### The pre-commit gate

Before any commit is marked done, all of these are true:

- [ ] The primary integration test for this commit was written first, was failing, now passes.
- [ ] The test crosses the boundary this commit claims to connect — it calls both sides.
- [ ] Every expected value in an assertion was derived from the spec, not from a prior run.
- [ ] Every new error variant has a test asserting the exact type.
- [ ] No test asserts only `.is_ok()` / `.is_err()` / existence.
- [ ] No test calls only code defined in this same commit.
- [ ] No bare panics, `unwrap()`, `todo!()`, or “unused” suppressions in committed code.
- [ ] The full suite passes with **zero warnings**.
- [ ] Exactly one boundary was crossed. No more.

**The final question:** if I introduced a plausible one-line bug, would a test catch it?
If no — the suite is incomplete, and the commit isn’t done.

-----

## Part IX — Recommended Practices

*Everything above is the canon, distilled from the standard. This section is my
recommendation: extensions that stay in the spirit of the method — small, explicit, and
deletable — without contradicting anything above. Adopt what earns its place; the
dependency gate (Part IV) applies to practices too.*

**Order the work by priority, ruthlessly.** Rule 10’s sequence (MVP → DX → Performance →
Features) is also a triage rule for any backlog. Performance work before the thing works,
or features before the developer experience is bearable, is effort spent in the wrong
order. Make it correct, make it pleasant to build on, make it fast, then — last — make it
do more.

**Measure before optimizing; budget from the spec.** Performance targets are numbers in
the spec, not vibes (e.g. “1000+ req/s,” “<1ms static response,” “<50MB total”). Profile
against the budget; don’t tune by intuition. An optimization with no measurement behind it
is a complexity increase with extra steps.

**Configuration is explicit and fails fast.** Read config from the environment, validate it
all at startup, and refuse to boot on anything missing or malformed. This is Rule 5 applied
to deployment: a service that starts in a half-configured state to fail mysteriously later
has hidden its assumptions. Never read secrets from source; never log them.

**Log events, not noise.** Structured key-value events at meaningful boundaries
(`server_started`, `request_failed`) beat free-text spew. A log line is a *why* comment for
the running system — it earns its place by being something you’d actually want during an
incident, not by narrating the obvious.

**Secure by default, validate at the edge.** “Rock solid” means deny-by-default and input
validated where it enters the system, before it reaches logic that trusts it. Path
traversal protection, auth on every protected route, and rejecting malformed input are MVP
features, not hardening to add later.

**Commit messages state the boundary.** Write the message in the imperative, name the seam
the commit connects, and reference the test that proves it. A reader should learn *which
boundary* and *how it’s verified* from the message alone — it’s the prose half of “one
boundary per commit.”

**Keep a short ARCHITECTURE record.** Comments explain local *why*; an architecture note
explains system *why* — the decisions and the roads not taken. One paragraph per real
decision beats a wiki nobody updates, and it answers the questions that otherwise become
the same Slack thread every six months.

**Prefer boring.** When two designs are equally minimal and equally correct, take the one
with fewer surprises. Cleverness is a cost the next reader pays. The whole method optimizes
for code that fits in your head; “boring” is what that feels like from the inside.

-----

*This standard embodies the method it describes: start minimal, ship working tools, prove
the approach by living inside it. Start with the Ten Rules. Everything else is elaboration.*
