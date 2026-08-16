# Development Notes

Read `MOTIVATION.md` for why this prototype exists, `docs/puri.md` for
the UI runtime decision and plan, `docs/model.md` for the data and
editor model decisions, and `docs/projections.md` for Grap's graph-
embedded evaluator and the retained, superseded wasm spike isolated in
`experiments/rust-wasm-projection`.

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
- The graph core has only two atoms: cell references and blobs. Record labels are always cell identities. UTF-8 text is the open `progred-text` record convention over a blob, not a primitive; a line projection recognizes the text or f64 facet even when the record has other fields.
- Resilient to invalid graph states — projections specify the happy path but must fall through gracefully to default/raw rendering; never crash or hide data on unexpected values
- Normal display is one explicit projection composed from ordered partial projections above one total structural fallback. Pass that projection explicitly through recursion; do not bury it in display context. Each partial is a function that checks its preconditions and fails closed. A partial returns a `progred_display::Layout` (boxes, display leaves, and optional `on_click`). The editor measures that into `Measured`; leaves become place-continuations and clicks become Puri handlers. Library packs offer those functions; the editor assembles one list from `stack::libraries` above the structural fallback. Raw is that fallback alone. `descend` receives the parent `Value` and one graph `Step`, extends stored provenance, and invokes the composed projection on that unresolved location. The projection owns lookup, so an absent field or element reaches the ordinary pending fallback. Starting the same projection at a transient root handles computed values; source provenance, not a separate derived operation, controls editability and selection attribution. A later, more specific partial can wrap an earlier facet (for example a unit around f64). A line's update receives the current value and the text, so it can keep extra fields or replace the value. Selection mounts a line editor only when the whole inert projection is one `LineEdit` (`stack::line`).
- Closed-record value projections may return an `editable_line` (text and f64 share that helper; they differ by update and affixes) or a larger layout. A line click carries the `LineEdit` so the selection does not rediscover it. Keyboard landings still ask `stack::line`. The structural walk is the total fallback and the live interpreter of those layouts. Focus and cursor live on the selection.
- Compile-time code generation must fail loudly — if the semantics-driven codegen returns, malformed graph data must produce a clear compile error, never be silently skipped
- Grap syntax is ordinary `Value` structure interpreted through library cell IDs. Core Grap owns only lambda/application, graph-valued closures and foreign-callable tags, stable evaluator absent identities, and bootstrap composition of graph and Rust implementations. F64, geometry, and CAD concepts belong to separate Grap libraries; never grow them into the evaluator.
- Grap evaluation is explicit in projections: `grap` is a value partial the grap crate offers, not a field hook in the structural walk and not evaluator syntax. A record with a `grap` field is shown as the stored expression (nested under that field, so its path stays `…+Key(grap)`), then `→`, then the returned `Value` projected from a transient, read-only root with this partial failing closed. Recognition is open, like text and f64: other fields do not block it, and an earlier partial in the editor's list wins if more than one matches. The arrow is projection chrome; Raw shows only the stored record. `evaluate` does not observe the field; a host that never loads the projection never sees it. Calls and lambdas elsewhere remain editable graph data; a returned call-shaped value is not evaluated again.
- Grap lambdas currently carry an ordered list of explicit parameter cells; calls put arguments directly under those cell labels. This is a concrete bootstrap shape, not a settled pattern language. Evaluating an untagged `{params, body}` lambda produces `{closure: {params, body, environment}}`. Grap-defined calls are strict and pure: every declared argument field is evaluated before the body runs, and their lexical environment is an ordinary graph record inside that closure value. Unrecognized records and lists are inert data and are never recursively searched for expressions.
- Graph conventions identify records by required positive evidence and are open to unrelated fields unless a domain explicitly defines a closed shape. Facet projections (text, f64) use that same open recognition so a wrapper can defer to them.
- In the normal projection, display named record fields first in alphabetical display-name order (CellId breaks equal-name ties), followed by unnamed fields in CellId order. Raw has no names, so it remains CellId-ordered.
- Cells resolve lexical binding first. A registered foreign-function cell evaluates to the explicit value `{ffi: cell}` without document/library resolution; the registry remains authoritative for the Rust implementation. Other cells resolve through document/library. Never key reference semantics on whether a cell currently has a stored value.
- Rust function implementations receive the call record, the calling `Environment` (bindings), and the live evaluation `Context` (fuel and the rest). They look up the fields they consume as raw argument expressions. They may inspect, return, forward, or recursively evaluate those expressions through the context in that environment or an explicitly derived one; ordinary strict Rust functions evaluate all of theirs. The environment becomes a graph record only when Rust explicitly hands it to Grap. Semantic failure remains an ordinary absent `Value`; the host `Result` only propagates evaluator halting such as exhausted fuel. Treat the registry as bootstrap machinery, not a second surface call syntax. A `Library` offers cells, foreign functions, and projections. Rust libraries own those offerings (a line projection lives in `progred-text` / `grap-f64`; the live `grap` field lives in the grap crate); the editor loads `stack::libraries` and merges them. A later foreign table overrides a shared cell. Grap-defined libraries can offer the same bag later. Each library absence is a stable library cell returned as a custom-null value, not a freshly minted occurrence; its library value structurally classifies it with the shared `isa: absent` convention and may carry more static metadata.
- Grap evaluation always produces a `Value`; missing cells, malformed syntax, cycles, and exhausted fuel are stable absent-cell values. Keep occurrence-specific host diagnostics alongside that result for tooling, but never use them as a second Grap control-flow channel.
- Evaluation itself has no quote/literal form. Raw Rust handlers already receive inert expressions, and a handler returning one returns data because function results are not evaluated again. The control library's ordinary Rust `quote` function walks its raw expression and replaces original `{unquote: expression}` leaves by evaluating their expressions in the quote caller's environment; it never revisits spliced results. Unquote-shaped records outside that traversal are ordinary data, and other ordinary data remains self-evaluating without quote. `evaluate` is likewise a registered function, not evaluator syntax.
- Grap's control library `case` evaluates one explicit subject once, tries ordered structural patterns, and evaluates only the first matching expression in the calling environment extended with its bindings (or the explicit default). Keep the subject local to that operation; do not introduce an implicit case-value binding. Record patterns are open, list patterns are exact and ordered, `{bind: cell}` captures, and repeated binders require equal captures. An absent result from a selected expression is still the result, not fallthrough.
- `isa` is a general Progred graph convention, independent of Grap and of any particular classification such as `absent`; keep it outside `progred-graph` so the data model stays primitive, while consumers own their class identities and use the shared relation.
- Well-known library cell IDs are once-minted random identities, never names or hashes of names; readable names are ordinary `progred-name` graph facts. Mint fixed identities from 16 unmodified OS-CSPRNG bytes (`new_cell_id`/`getrandom`, or `openssl rand -hex 16` when producing a source literal), never from a UUID generator.
- `CellId` is an opaque 128-bit identity, not an RFC UUID: all bits are random, with no version or variant fields. Preserve its canonical 32-hex gid spelling and existing hyphenated Serde spelling.
- Until document files exist outside this repository, change GID and its checked-in files in lockstep. Do not add file-format versions, migration branches, or compatibility readers.
- Evaluation is fueled and dependency-reporting. A `grap` wrapper projects absent values like every other transient result; Raw always exposes its stored expression.

## Testing

- Test pure logic directly; don't write UI tests that just verify strings pass through to draw calls
- Pure `render`/`update` functions mean snapshot/property tests on draw-list data need no windowing harness — prefer them; use vello headless readback only when a visual golden is genuinely needed
- Visual changes are checkable without launching the app: `cargo test -p progred svg_bench` renders the sample and focused Grap demo through the real projection to `target/raw_projection.svg` and `target/grap_demo.svg`, and `cargo run -p puri --example delimiter_bench` renders the drawn-delimiter family against the font's own outlines; view any of them with `qlmanage -t -s 1600 -o <dir> <svg>`

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
