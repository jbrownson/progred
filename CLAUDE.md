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
xcodebuild -scheme progred -destination 'platform=macOS' build
xcodebuild -scheme progred -destination 'platform=macOS' test
```

Note: xcodebuild requires sandbox to be disabled for Swift Package Manager cache access.

## Architecture

- `D` enum is the display language (`Display/D.swift`)
- `ProjectionContext` + dispatch chain projects graph → D (`Display/Projection.swift`)
- Layered projections: domain (`Display/DomainProjection.swift`) → kernel (`Display/KernelProjection.swift`) → raw
- `DView` renders D → SwiftUI (`Display/DView.swift`)
- `descend(field)` looks up an edge and dispatches; `projectChild(entity)` dispatches a known UUID
- `renderRef` renders shallow type references (just names); full `descend` renders declarations
- Lists (cons/empty) need custom handling — their graph structure doesn't match the editing/display structure

## Key Design Rules

- **Documents are pure graph structure** — No semantic interpretation baked in. The graph is a dumb substrate.
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
- Prefer free functions with explicit parameters over methods when `self` isn't needed
- Apply Haskell-style thinking (explicit data flow, pure function composition) but idiomatic Swift syntax
- `guard` for preconditions is idiomatic — use it freely for early returns
- Prefer ternary expressions over if/else when returning or assigning a value based on a condition
- Name constants that are repeated or related to other values; express relationships explicitly (one as a function of the other). Inline one-off values are fine.
- Dead code should be deleted, not commented out
- Push back if something seems wrong
