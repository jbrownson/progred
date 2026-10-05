# Development Notes

Read `MOTIVATION.md` for why this implementation exists, `docs/gid.md` for
the native data substrate, `docs/puri.md` for the UI runtime decision and plan, `docs/model.md` for the data and
editor model decisions, and `docs/projections.md` for Grap's GID-
embedded evaluator.

These documents describe the current implementation, not verified statements
of the owner's intent. See `docs/README.md` for the reading map. Superseded
notes belong in `docs/history/`; keep proposals and deferred work separate
from current contracts, and do not infer owner approval from old prose.

## Cargo

Cargo build scripts and procedural macros execute dependency code. The
checked-in `.cargo/config.toml` refuses ordinary Cargo builds and updates as an
accidental-use tripwire. Do not bypass it by clearing `RUSTC_WRAPPER` manually.

Use the repository's macOS Seatbelt wrapper, which has its own Cargo home and
target directory under `target/sandbox`:

```bash
make sandbox-check
make sandbox-test
make sandbox-build
./tools/sandbox-cargo <cargo command> [arguments...]
```

Use `make run` in place of `cargo run --release`. On macOS it builds under
Seatbelt, packages and ad-hoc signs the app with its App Sandbox entitlements,
launches a fresh instance, and waits for it to exit. On Linux the checked-in
`tools/run-linux` launcher intentionally performs an ordinary locked Cargo run;
there is no repository-provided Linux sandbox yet. Agents still must not launch
the app; the user runs and visually tests it. See `docs/build-security.md` for
the boundary and the separate fetch/update commands.

Web builds stay on macOS, under Seatbelt; do not add a Linux browser build.
Once Rust 1.100 is stable (12 November 2026), switch `sandbox-cargo update`
and `resolve` to stable Cargo: `registry.global-min-publish-age` no longer
needs `-Z min-publish-age` there. The browser build still needs a nightly for
`-Z build-std`, which has no scheduled stabilization, so `web-threaded` keeps
`nightly-2026-08-27`.

## Workflow

- Don't run the app — the user prefers to run and test it themselves
- Don't fight the system — avoid hacks/workarounds that go against how frameworks or platforms are designed; push back early when something seems like it's not meant to work that way
- Keep unrelated changes in separate commits whenever possible; avoid bundling housekeeping with feature work
- Only commit when explicitly asked
- When a design pattern or lesson emerges during work, propose additions to this document so future sessions start with that knowledge

## Puri Rules

Details and rationale live in `docs/puri.md`; navigation in
`docs/navigation.md`.

- Widgets are pure functions (persistent widget state, props) → (draw calls, handlers). Puri holds nothing between frames, mints no identity, retains no hierarchy.
- Hover mirrors painting: a claim names a target or occludes, and probes run over settled placements in painting order. New pointer positions resolve through the same retained probes and precedence; never add a second approximate hover algorithm or store a resolved hover as an input to building a frame.
- Activation and picking lower into the same front-to-back pointer handler chain as raw input; never dispatch them as separate phases. The shell supplies the settled hover and its owning view explicitly at dispatch.
- Event acceptance controls propagation; gesture startup is an explicit update to caller-owned state. Never infer a gesture from an unrelated handler returning true.
- Projection gesture continuations have one caller-owned active slot. The accepting handler installs the continuation, which owns motion, domain updates, and undo grouping until release or cancellation.
- Batch continuous pointer input before rebuilding a frame, preserving observed samples in order; release and cancellation flush pending motion first. No per-widget batching flag, and the shell never discards samples.
- Durable widget helper state contains only caller-owned content and cross-frame interaction data; presentation inputs belong to the current description.
- Platform services are caller-supplied capabilities in dispatch context; Puri never constructs clipboard, clock, or window services itself.
- Focus is an input: the app owns who has focus and any focus order.
- No hidden framework caches. Cross-frame reuse is caller-owned: threaded text shaping and the `incremental` computation graph. Every changed frame input remints and presents a whole frame; never add event-specific relevance tests or partial invalidation. If that becomes too slow, the answer is one general dependency-tracked invalidation system.
- Puri is layout-neutral: it owns the `Placement` geometry contract, while the consumer owns measurement composition, layout nodes, containers, and traversal. Do not make a particular layout algebra part of a Puri widget's type.
- Navigation derives from the chosen layout: projections declare stops, and Progred's row/column layout orders them into logical lines. No pixel scoring, second navigation graph, independent neighbor routing, or navigation state in Puri.
- Progred's layout is the baseline box algebra plus ordered layout alternatives (first natural form that fits wins, the last accommodates); no general layout engine. Keep measurement and placement separate, and add no document traversal or control variants to `Layout`.
- Keep layout honest about painted geometry: derive anchors and clearance from the same metrics, outlines, and stroke widths used for drawing. Intentional padding and gaps are explicit styling inputs; do not tune separate offsets to compensate for geometry owned elsewhere.
- Every placement callback receives an explicit `Placement` (`rect`, `available_rect`, `clip_rect`). Never thread clipping as mutable context; available space is neither a clip nor a hit target.
- Masonry is a quarry, not a foundation: vendor a high-value file only with attribution and purify it in place; rewrite trivial widgets; never inherit its tree, pods, or ctx protocol.
- Keep the Puri runtime separate from widget catalogs. Reusable composed widgets belong in the sibling `puri-widgets` package and remain pure consumers of Puri; Progred owns document adaptation, layout, and popup policy.
- Progred-owned widgets may work directly with the editor: capture props and locations in transient handlers and pass `&mut Editor` at dispatch. Do not add callback dictionaries or artificial crates just to keep them generic. Puri itself stays editor-independent.
- Extend Puri only as Progred needs it.
- Inert decorators construct no editing state or capabilities; widget preparation requests document-site state only when needed. Hover claims and visual feedback are independent, explicitly composed decorators.

## Key Design Rules

Details and rationale live in `docs/projections.md` (Grap and
projections), `docs/model.md` (selection, completion, panes), and
`docs/gid.md` (the data substrate).

- Prefer simple constructs and reusable combinators that build layers of abstraction. A composition should remain an ordinary input to further composition: partial projections combine into a partial projection, completion providers into a provider. Build higher-level behavior through these interfaces rather than adding domain-specific cases to central machinery. Lowering may optimize the representation and execution of those layers, but must preserve their meaning and observable behavior. See [MOTIVATION.md](MOTIVATION.md#simple-constructs-and-combinators).
- GID is the native logical model and future binary storage stack, not a textual format. `name`, text, and Grap's absence values are libraries above the data layer, never GID features. The binder notation and `*.gid` fixtures are a temporary text bridge.
- The GID core has only two atoms: cell references and blobs. Record labels are always cell identities; UTF-8 text is a library record convention over a blob.
- Resilient to invalid GID states — projections specify the happy path but must fall through gracefully to default/raw rendering; never crash or hide data on unexpected values.
- Controls supply sensible defaults for absent selection payload fields. Selecting a writable missing location enters its picker without an explicit pending marker; keep explicit query modes for intentional requests, not as required setup.
- Normal display is one explicit projection: ordered partial projections above one total structural fallback, passed explicitly through recursion and never buried in display context. Each partial checks its preconditions and fails closed. Puri leaves never acquire document locations, selection payloads, editing rules, or event policy; there is no parallel Progred drawing language and no central action enum or reducer.
- Documents currently contribute no library cells, foreign functions, or projections. Do not discover configuration through fixed-address cells or other data outside ordinary root reachability.
- Pane entries are ordinary values in the document's root-reachable side lists; opening, moving, and deleting panes edit those lists and participate in history.
- Completion is an ordinary native widget placed by the projection that renders the pending location. Providers receive one explicit request and are never hidden in traversal context; applicability decisions stay in libraries, not host hooks. Retain the exact visible offers in the placed frame so hover and commit never reconstruct a different list.
- Text and number partials call the stock native line widget with a conversion callback owned by the current handler. The caller runs each editing operation, then converts and records undo at that boundary; there is no shell-wide write-through. Line-editor state stays Rust-owned beside the selection payload; never retain and synchronize a second copy.
- Compile-time code generation must fail loudly — malformed GID data must produce a clear compile error, never be silently skipped.
- Grap syntax is ordinary `Value` structure interpreted through library cell IDs. Privileged knowledge of a library convention (such as unboxed f64) is an accelerator only, never a capability: each such library must remain expressible externally and read to Grap as an ordinary library.
- Grap's `{value: expression}` wrapper evaluates its expression in the calling environment; it introduces neither a binding nor memoization.
- The structural fallback projects cells deeply; Grap expression and callable positions request a shallow named-cell projection. Use explicit composition rather than teaching the raw structural fallback about Grap.
- Put more specific domain projections before general ones. Malformed shapes decline so a general projection still exposes every value.
- Grap evaluation is explicit in projections: `evaluate` is a value partial the Grap library offers, not a field hook in the structural walk and not evaluator syntax.
- Grap lambdas carry an ordered list of explicit parameter cells, a bootstrap shape rather than a settled pattern language. Calls are strict; effects are explicit foreign calls and scoped capabilities. Unrecognized records and lists are inert data, never searched for expressions.
- GID conventions identify records by required positive evidence and are open to unrelated fields. Missing or malformed required contents are an ordinary partial failure: try the next projection.
- In the normal projection, display named record fields first in alphabetical display-name order (CellId breaks equal-name ties), followed by unnamed fields in CellId order. Raw has no names, so it remains CellId-ordered.
- Cells resolve lexical binding first, then one definition: the document's, otherwise the first loaded library's. Definitions are transparent, evaluated where the cell is referenced. Duplicate definitions are tolerated, not an overloading mechanism. Stored `Follow` paths name a stable source, never a stack ordinal.
- Rust functions receive raw argument expressions, the calling environment, and the evaluation context. Semantic failure is an ordinary absent `Value`; the host `Result` only propagates halts. Observable writes go through `Context::effect`; there are no snapshots or rollback, and explicit decline after an effect halts. Fuel is never restored.
- Grap evaluation returns an owned `RuntimeValue`; materialize GID only through explicit adapters. Every absence is an open `{absent: reason-cell}` value with a stable reason cell; code tests the CellId, never text, and there is no parallel Rust diagnostic channel.
- Evaluation has no quote/literal form; `quote` and `evaluate` are ordinary registered functions.
- Grap's `match` evaluates its subject once and evaluates only the first matching case; a selected absent result is still the result. Do not introduce an implicit match-value binding.
- Absence composition belongs only to explicit ordered-choice operations, never to cell lookup or an ambient failure trace.
- Well-known library cell IDs are once-minted random identities, never names or hashes of names; readable names are ordinary `libraries::name` GID facts. Mint fixed identities from 16 unmodified OS-CSPRNG bytes (`new_cell_id`/`getrandom`, or `openssl rand -hex 16` when producing a source literal), never from a UUID generator.
- `CellId` is an opaque 128-bit identity, not an RFC UUID: all bits are random, with no version or variant fields. Every text spelling, including `Display` and human-readable Serde, is 32 lowercase hex digits; there is no hyphenated form.
- Until text-bridge documents exist outside this repository, change its parser and checked-in `*.gid` fixtures in lockstep. Do not add versions, migration branches, or compatibility readers.
- Fuel is a per-evaluation responsiveness guard, not a security boundary. Do not propagate unused fuel through projected values or add cross-projection budget clamps.

## Testing

- Test pure logic directly; don't write UI tests that just verify strings pass through to draw calls
- Pure `render`/`update` functions mean snapshot/property tests on draw-list data need no windowing harness — prefer them; use vello headless readback only when a visual golden is genuinely needed
- Visual changes are checkable without launching the app: `./tools/sandbox-cargo test -p progred svg_bench` renders the sample and focused Grap demo through the real projection to `target/raw_projection.svg` and `target/grap_demo.svg`, and `./tools/sandbox-cargo run -p puri --example delimiter_bench` renders the drawn-delimiter family against the font's own outlines; view any of them with `qlmanage -t -s 1600 -o <dir> <svg>`

## Code Style

- Very limited comments — code should be self-documenting
- Expression-oriented where possible
- Prefer long expressions broken across multiple lines over multiple statements with intermediate names — naming is hard, avoid unnecessary names
- Exception: extract helper functions when intermediate steps represent distinct semantic concepts — top-level function becomes a readable composition of named transformations (functional decomposition)
- Prefer free functions with explicit parameters over methods when `self` isn't needed — makes inputs/outputs clear, easier to unit test, enables composition in a single method that has access to `self` (Haskell-style)
- Look for generic abstractions — extract patterns in how computations combine and data flows (the way `fold`/`map`/monads abstract over structure, not specific operations)
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
