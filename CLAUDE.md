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

## Key Design Rules

- **Documents are pure graph structure** — No semantic interpretation baked in. The graph is a dumb substrate.
- **Resilient to invalid graph states** — The graph can contain anything. Projections specify the happy path but must fall through gracefully. Never assume what's at an edge; make the good case easy but don't crash or hide data on the bad case.
- **`record` edge is the value head** — Every value node's schema head is determined by its `record` edge (kernel convention). This replaces `isa` from the Rust prototype.
- **Type parameters as edge labels on Apply** — Apply uses the actual Type Parameter node UUIDs as edge labels, not positional lists. This makes type application naturally map-shaped.
- **Self-describing schema** — The type system defines itself in the same graph. Record describes Record, Field describes Field, etc.

## Code Style

- Very limited comments — code should be self-documenting
- Expression-oriented where possible
- Prefer long expressions broken across multiple lines over multiple statements with intermediate names
- Exception: extract helper functions when intermediate steps represent distinct semantic concepts
- Look for generic abstractions — extract patterns in how computations combine and data flows
- Dead code should be deleted, not commented out
- Push back if something seems wrong
