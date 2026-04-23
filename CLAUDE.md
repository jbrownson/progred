# Development Notes

Read `README.md` for project philosophy and architecture.

## Workflow

- Don't run the app — the user prefers to run and test it themselves
- Don't fight the system — avoid hacks/workarounds that go against how frameworks or platforms are designed; push back early when something seems like it's not meant to work that way
- Keep unrelated changes in separate commits whenever possible; avoid bundling housekeeping with feature work
- Only commit when explicitly asked
- When a design pattern or lesson emerges during work, propose additions to this document so future sessions start with that knowledge

## Building and Testing

```bash
xcodebuild -project prototype-swift/progred.xcodeproj -scheme progred -destination 'platform=macOS' build
xcodebuild -project prototype-swift/progred.xcodeproj -scheme progred -destination 'platform=macOS' test
```

Note: xcodebuild requires sandbox to be disabled for Swift Package Manager cache access. Pass `-project` instead of `cd`-ing into the subdirectory; bash `cd` persists across tool calls and will silently break later relative paths (e.g. `git add`).

## Architecture

- `D` enum is the display language (`Display/D.swift`)
- `ProjectionContext` + dispatch chain projects graph → D (`Display/Projection.swift`)
- Layered projections: domain (`Display/DomainProjection.swift`) → kernel (`Display/KernelProjection.swift`) → raw
- `Reconcilable` protocol for D → AppKit view reconciliation (`Display/DViews/Reconcilable.swift`)
- `descend(field)` looks up an edge, projects through the dispatch chain, and wraps in `Descend` — handles missing values (placeholder via raw fallback), cycles, and editability (`commit == nil` means read-only)
- `renderRef` renders shallow type references (just names); full `descend` renders declarations
- Lists (cons/empty) need custom handling — their graph structure doesn't match the editing/display structure

## Key Design Rules

- **Documents are pure graph structure** — No semantic interpretation baked in. The graph is a dumb substrate.
- **All ids are eternal nodes** — Primitive ids (strings, numbers) are not created or destroyed; they simply exist in the universe. Editing a string field doesn't mutate a string node — it repoints the parent's edge at a different string id. `readOnly` on a node means its edges can't be mutated, but the parent's edge pointing *to* it can still be replaced (`descend` separates `edgeCommit` from child context commit).
- **Resilient to invalid graph states** — The graph can contain anything. Projections specify the happy path but must fall through gracefully. `descend` handles missing edges (placeholder) and unexpected values (default rendering) automatically. Never assume what's at an edge; make the good case easy but don't crash or hide data on the bad case.
- **Dispatch on record type, not edge presence** — Check `ctx.record() == schema.someRecord`, not "has fields edge therefore is a record." Duck typing breaks when unrelated nodes share edge labels.
- **`record` edge is the value head** — Every value node's schema head is determined by its `record` edge (kernel convention). This replaces `isa` from the Rust prototype.
- **Type parameters as edge labels on Apply** — Apply uses the actual Type Parameter node UUIDs as edge labels, not positional lists. This makes type application naturally map-shaped.
- **Self-describing schema** — The type system defines itself in the same graph. Record describes Record, Field describes Field, etc.

## Code Style

- Very limited comments — code should be self-documenting; use `// MARK: -` for section navigation in Xcode but only when grouping non-obvious things, not echoing adjacent function/type names
- Expression-oriented where possible
- Prefer long expressions broken across multiple lines over multiple statements with intermediate names
- Exception: extract helper functions when intermediate steps represent distinct semantic concepts
- Look for generic abstractions — extract patterns in how computations combine and data flows
- Factor generic algorithms from concrete operations — parameterize with closures, keep the algorithm free of domain types (e.g., `reconcile<T, Ts>` takes closures for replace/append/remove, knows nothing about NSView)
- Prefer composable combinators over monolithic functions — small functions that transform or wrap behavior (e.g., `orFilter`, `caseInsensitive`, `sortedFilter`) let you build complex pipelines that read as a declaration of intent
- Prefer `zip`, `dropFirst`, `enumerated`, `forEach`, `map` over index arithmetic, `stride`, and manual `for i in 0..<n` loops
- Prefer free functions with explicit parameters over methods when `self` isn't needed
- Apply Haskell-style thinking (explicit data flow, pure function composition) but idiomatic Swift syntax
- `guard` for preconditions is idiomatic — use it freely for early returns
- Prefer ternary expressions over if/else when returning or assigning a value based on a condition
- Name constants that are repeated or related to other values; express relationships explicitly (one as a function of the other). Inline one-off values are fine.
- Use consistent naming across abstraction levels — if the generic algorithm uses `reconcile`, the concrete wrappers and protocol methods should too, not `resolve` or `update`
- Don't introduce words without clear meaning — every term in a name should pull its weight
- Use the same name for the same concept within a scope — don't alias (e.g., `conses` in one place, `elements` in another for the same data)
- Be sparing with default arguments — only when the default is a genuinely reasonable "most of the time" value that is occasionally overridden, not just to save typing at one call site
- Dead code should be deleted, not commented out
- Push back if something seems wrong
