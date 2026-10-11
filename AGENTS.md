# Agent Guidance

How to work on Progred. Design and implementation details live in the docs and
the code; this file points to them rather than repeating them.

## Key files

- `MOTIVATION.md`: why this implementation exists, and its design style of
  simple constructs and combinators.
- `docs/README.md`: the reading map. The core references are `docs/gid.md` (the
  data substrate, including how to mint cell IDs), `docs/gid-text.md` (the text
  bridge), `docs/model.md` (selection, completion, panes, history),
  `docs/projections.md` (Grap and projections), `docs/puri.md` (the UI runtime
  and editor frame), `docs/navigation.md`, and `docs/incremental.md`
  (dependency-tracked computations).
- `docs/build-security.md`: the build sandbox and dependency policy.
- `vendor/README.md`: vendored dependencies and their local patches.
- `docs/deferred.md`, `docs/release-checklist.md`, and `docs/macos-followups.md`:
  open work.
- `docs/history/`: superseded notes and dated measurements. Not instructions.

The docs describe the current implementation, not verified statements of the
owner's intent, and much of their prose is assistant-written. When a doc and
the code disagree, investigate rather than restoring an older design. Keep
proposals and deferred work separate from current descriptions, and move
superseded notes to `docs/history/`.

## Working

- Push back if something seems wrong.
- Don't fight the system: avoid hacks and workarounds that go against how a
  framework or platform is designed, and push back early when something doesn't
  seem meant to work that way.
- Fix the source of a mismatch instead of compensating for it downstream, for
  example with a hand-tuned offset.
- Commit when it makes sense, and keep unrelated changes in separate commits;
  don't bundle housekeeping with feature work. Before pushing, run
  `cargo fmt --all -- --check` and `make test`. When pushing to a session
  branch, add commits as work proceeds and rewrite its history only when asked.
- Don't run the app for the user to test; they run and test it themselves.
  Running it for your own checks is fine, preferably headlessly.
- When a pattern or lesson emerges, propose an addition to this file. Don't add
  it without asking.

## Building

Cargo build scripts and procedural macros execute dependency code. The
checked-in `.cargo/config.toml` refuses ordinary Cargo builds as an
accidental-use tripwire; don't bypass it by clearing `RUSTC_WRAPPER` by hand.
Use the Make targets, which choose the platform's wrapper:

```bash
make check
make test
make build
make run
```

On macOS they run under the repository's Seatbelt wrapper; use
`./tools/sandbox-cargo <cargo command> [arguments...]` for anything else. Linux
has no repository sandbox yet, and `./tools/linux-cargo` is its single
exception to the tripwire. See `docs/build-security.md`.

The browser build is supported on macOS. Supporting it elsewhere isn't worth its
complexity yet, but don't block other platforms on purpose;
`website/build-ci.sh` builds it on Linux for publishing.

Update dependencies through `./tools/sandbox-cargo update`, which enforces a
minimum publish age. Patching a dependency is sometimes the right call, but keep
an eye on every patch: `vendor/README.md` lists them, and each should be checked
against upstream and dropped or proposed upstream when it can be.

## Pre-alpha stance

- No compatibility burden yet. The GID text bridge has no versions, migration
  branches, or compatibility readers; change its parser and the checked-in
  `*.gid` fixtures together.

## Principles

- Prefer simple constructs and reusable combinators. A composition should stay
  an ordinary input to further composition; build richer behavior from those
  pieces rather than adding special cases to central machinery. Lowering may
  optimize how a composition runs, but must not change what it means.
- Widgets are pure functions of caller-owned state and props, producing drawing
  and handlers. Puri holds nothing between frames, mints no identity, and
  retains no hierarchy.
- No hidden caches. Reuse across frames goes through the dependency-tracked
  computations in `incremental`, never an ad-hoc cache or an event-specific
  relevance test. If something is too slow, improve that general mechanism.
- Event handling is distributed and compositional: handlers describe what
  happened and decide for themselves whether to act. There is no central action
  enum or single reducer.
- Be resilient to invalid data: projections handle the expected shapes and fall
  back gracefully otherwise, never crashing or hiding data.
- Things should do something reasonable with no special setup. For example, any
  location can be selected, with controls supplying sensible defaults.

## Testing

- Test pure logic directly; don't write UI tests that only check that strings
  reach draw calls.
- Prefer tests over draw-list data, which need no window; use GPU readback only
  when a visual golden is genuinely needed.
- Drive interactions headlessly with `crate::driver::Driver`: it clicks, types,
  and reads by document path through the shell's own event translation.
- Visual changes are checkable without launching the app: `make test` runs
  `svg_bench`, which renders the sample and the Grap demo to SVG in the build
  directory, and running the `delimiter_bench` example of `puri` through the
  platform's Cargo wrapper renders the drawn delimiters against the font's
  outlines. Rasterize them with `rsvg-convert`.

## Code Style

- Prefer few comments and self-documenting code, but add a comment where it
  helps a reader or a model understand something non-obvious, and keep comments
  accurate when the code changes.
- Expression-oriented where possible.
- Prefer long expressions broken across multiple lines over multiple statements
  with intermediate names — naming is hard, avoid unnecessary names.
- Exception: extract helper functions when intermediate steps represent distinct
  semantic concepts, so the top-level function reads as a composition of named
  transformations.
- Prefer free functions with explicit parameters over methods when `self` isn't
  needed: inputs and outputs stay clear, they're easier to test, and they compose.
- Look for generic abstractions: patterns in how computations combine and data
  flows, the way `fold`, `map`, and monads abstract over structure.
- Apply Haskell-style thinking (explicit data flow, pure function composition)
  in idiomatic Rust; don't fight the language.
- Factor out common assignments: `x = if cond { a } else { b }`, not
  `if cond { x = a } else { x = b }`.
- Functional style: iterator chains, `try_fold`, `filter_map`, and
  `std::array::from_fn` over mutable accumulators and loops where it doesn't
  make things worse.
- Avoid `let mut` when a functional alternative is equally clear.
- No `isX()` predicate methods; use `matches!` or pattern matching at the call
  site.
- Eliminate partial functions: use `split_first`/`split_last` over manual
  indexing.
- Return references for non-trivial types and let the caller decide to clone;
  methods on small structs allow disjoint borrows where methods on the parent
  wouldn't.
- Avoid early returns: prefer `if let`, `match`, or expression-oriented
  alternatives over `let-else return` or `return` in closures.
- Delete dead code; don't comment it out.
