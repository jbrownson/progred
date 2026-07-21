# Development Notes

Read `MOTIVATION.md` for why this prototype exists, `docs/puri.md` for
the UI runtime decision and plan, and `docs/model.md` for the data and
editor model decisions.

## Cargo Build Cache

Cursor's sandbox sets `CARGO_HOME` and `RUSTUP_HOME` to temporary directories. This causes cache invalidation when alternating between terminal and Cursor builds.

When running cargo commands, unset these:

```bash
unset CARGO_HOME RUSTUP_HOME && cargo build
```

## Workflow

- Don't run the app — the user prefers to run and test it themselves
- Don't fight the system — avoid hacks/workarounds that go against how frameworks or platforms are designed; push back early when something seems like it's not meant to work that way
- Keep unrelated changes in separate commits whenever possible; avoid bundling housekeeping with feature work
- Only commit when explicitly asked
- When a design pattern or lesson emerges during work, propose additions to this document so future sessions start with that knowledge

## Git Commits

zsh heredocs fail in the sandbox because zsh writes temp files to `$TMPPREFIX` (defaults to `/tmp/zsh`), which is outside the sandbox allowlist. Fix by exporting it before the commit:

```bash
export TMPPREFIX=/tmp/claude/zsh && git commit -m "$(cat <<'EOF'
Commit message here
EOF
)"
```

## Puri Rules

- Widgets are pure functions (persistent widget state, props) → (draw calls, handlers). Puri holds nothing between frames, mints no identity, retains no hierarchy.
- Focus is an input: the app owns who has focus and tab order; helpers are pure and advisory. The focused text widget emits an IME caret rect as output.
- No framework caches. If profiling demands one, it is a caller-threaded memo table for a pure function (text shaping first, most likely), never hidden state.
- Layout is the baseline box algebra plus Wadler-style grouping; no general layout engine. Keep measurement and placement separate in the placement interface.
- Masonry is a quarry, not a foundation: vendor high-value files (text input first) with attribution and purify in place; rewrite trivial widgets; never inherit its tree, pods, or ctx protocol.
- Extend Puri only as Progred needs it.

## Key Design Rules

- Documents are structural values plus the cell table; a cell's NAME is identity metadata in the data layer (2026-07-20) — everything else semantic (isa and friends) stays editor conventions, never data-layer features
- Resilient to invalid graph states — projections specify the happy path but must fall through gracefully to default/raw rendering; never crash or hide data on unexpected values
- Compile-time code generation must fail loudly — if the semantics-driven codegen returns, malformed graph data must produce a clear compile error, never be silently skipped

## Testing

- Test pure logic directly; don't write UI tests that just verify strings pass through to draw calls
- Pure `render`/`update` functions mean snapshot/property tests on draw-list data need no windowing harness — prefer them; use vello headless readback only when a visual golden is genuinely needed
- Visual changes are checkable without launching the app: `cargo test -p progred svg_bench` renders the sample document through the real projection to `target/raw_projection.svg`, and `cargo run -p puri --example delimiter_bench` renders the drawn-delimiter family against the font's own outlines; view either with `qlmanage -t -s 1600 -o <dir> <svg>`

## Code Style

- Very limited comments — code should be self-documenting
- Expression-oriented where possible
- Prefer long expressions broken across multiple lines over multiple statements with intermediate names — naming is hard, avoid unnecessary names
- Exception: extract helper functions when intermediate steps represent distinct semantic concepts — top-level function becomes a readable composition of named transformations (functional decomposition)
- Prefer free functions with explicit parameters over methods when `self` isn't needed — makes inputs/outputs clear, easier to unit test, enables composition in a single method that has access to `self` (Haskell-style)
- Look for generic abstractions — extract patterns in how computations combine and data flows (the way `fold`/`map`/monads abstract over structure, not specific operations)
- Dispatch on node type via `try_wrap`, not edge presence — checking for a marker edge to mean "is a sum" is duck typing that breaks if an unrelated node shares that edge label
- Apply Haskell-style thinking (explicit data flow, pure function composition) but idiomatic Rust syntax — don't fight the language
- Factor out common assignments: `x = if cond { a } else { b }` not `if cond { x = a } else { x = b }`
- Functional style: iterator chains, `try_fold`, `filter_map`, `std::array::from_fn` over mutable accumulators and loops where it doesn't make things worse
- Avoid `let mut` when a functional alternative is equally clear
- No `isX()` predicate methods — use `matches!` or pattern matching at the call site
- Eliminate partial functions: use `split_first`/`split_last` over manual indexing
- Return references for non-trivial types (let caller decide to clone); methods on small structs enable disjoint borrow checking over methods on the parent
- Avoid early returns — prefer `if let`, `match`, or expression-oriented alternatives over `let-else return` / `return` in closures
- Dead code should be deleted, not commented out
- Events describe what happened (user actions), not what to do about it; interpret them in one place after rendering
- Push back if something seems wrong
