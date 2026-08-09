# Development Notes

Read `MOTIVATION.md` for why this prototype exists, `docs/puri.md` for
the UI runtime decision and plan, `docs/model.md` for the data and
editor model decisions, and `docs/projections.md` for Grap's graph-
embedded evaluator and the retained, superseded wasm spike.

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
- Durable widget helper state contains only caller-owned content and cross-frame interaction data. Font, paint, affixes, focus, placeholder, and similar presentation inputs belong to the current description and are captured by its transient handlers.
- Platform services are caller-supplied capabilities materialized in dispatch context; Puri must not construct global clipboard, clock, or window services itself.
- Focus is an input: the app owns who has focus and tab order; helpers are pure and advisory. The focused text widget emits an IME caret rect as output.
- No framework caches. If profiling demands one, it is a caller-threaded memo table for a pure function (text shaping first, most likely), never hidden state.
- Puri is layout-neutral: it owns the `Placement` geometry contract, while the consumer owns measurement composition, layout nodes, containers, and traversal. Puri text and editor descriptions expose metrics and accept a settled placement; interaction helpers register against one. Do not make a particular layout algebra part of a Puri widget's type.
- Progred's layout is the baseline box algebra plus Wadler-style grouping; no general layout engine. Keep measurement and placement separate. Use `around` when a Progred wrapper must control whether or when its subtree places; derive ordinary leading work with `before` rather than adding special cases.
- Every placement callback receives an explicit `Placement`: the widget's full `rect` and the effective enclosing `clip_rect` (the intersection of ancestor axis-aligned layout clips, not pre-intersected with the widget). Ordinary children inherit it unchanged; an actual clipping container intersects its bounds into it. Never thread clipping as mutable context. Hover and gesture starts must be inside both rects; motion and release for an active gesture stay unbounded. Canvas clips remain a separate, arbitrary-shape drawing concern.
- Masonry is a quarry, not a foundation: vendor high-value files (text input first) with attribution and purify in place; rewrite trivial widgets; never inherit its tree, pods, or ctx protocol.
- Extend Puri only as Progred needs it.

## Key Design Rules

- Documents are structural values plus a direct `CellId -> Value` table; an absent entry is a bare cell. `name`, `isa`, and similar meanings are optional ordinary graph conventions, never data-layer features.
- The graph core has only two atoms: cell references and blobs. Record labels are always cell identities. UTF-8 text is the open `progred-text` record convention over a blob, not a primitive; a compact projection may hide that record only when it has no extra fields.
- Resilient to invalid graph states — projections specify the happy path but must fall through gracefully to default/raw rendering; never crash or hide data on unexpected values
- Compile-time code generation must fail loudly — if the semantics-driven codegen returns, malformed graph data must produce a clear compile error, never be silently skipped
- Grap syntax is ordinary `Value` structure interpreted through library cell IDs. Core Grap owns only the function-definition/call vocabulary and a generic foreign-function registry. F64, geometry, and CAD concepts belong to separate Grap libraries with host implementations registered as needed; never grow them into the evaluator.
- Graph conventions identify records by required positive evidence and are open to unrelated fields unless a domain explicitly defines a closed shape. Keep semantic recognition separate from whole-record presentation: a compact stand-in may replace a record only when it accounts for every field it would hide.
- Cells resolve lexical binding first, then registered foreign functions, then document/library; never key reference semantics on whether a cell currently has a stored value.
- Foreign functions map evaluated `Value` arguments to one `Value`; do not impose Rust's `Result` distinction at the boundary. Each library failure mode is a stable library cell returned as a custom-null value, not a freshly minted occurrence; its library value structurally classifies it with the shared `isa: error` convention and may carry more static metadata.
- `isa` is a general Progred graph convention, independent of Grap and of any particular classification such as `error`; keep it outside `progred_graph` so the data model stays primitive, while consumers own their class identities and use the shared relation.
- Well-known library cell IDs are once-minted random identities, never names or hashes of names; readable names are ordinary `progred-name` graph facts.
- `CellId` is an opaque 128-bit identity, not an RFC UUID: all bits are random, with no version or variant fields. Preserve its canonical 32-hex gid spelling and existing hyphenated Serde spelling.
- Until document files exist outside this repository, change GID and its checked-in files in lockstep. Do not add file-format versions, migration branches, or compatibility readers.
- Evaluation is fueled and dependency-reporting. A failed domain projection declines to Raw rendering; invalid programs never hide their underlying graph structure.

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
