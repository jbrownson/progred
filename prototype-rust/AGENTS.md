# Development Notes

Read `README.md` for project philosophy, architecture, and current status.

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

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
EOF
)"
```

## Testing

- **egui_kittest** (0.33.x): Runs egui headlessly, exposes AccessKit tree for querying widgets by label/role, simulating clicks/keypresses, and snapshot testing (with `wgpu` feature). Evaluated 2026-03-08 — works but not useful yet because our architecture cleanly separates logic from rendering. Revisit for snapshot regression testing on projections once visual stability matters.
- **Test pure logic, not egui wiring**: `build_entries`/`filter_entries` are pure functions tested directly. Don't write kittest tests that just verify egui passes strings through to buttons.
- **`Uuid::new_v4()`** for tests needing raw nodes: `progred_graph` re-exports `uuid::Uuid`. Use `Uuid::new_v4()` directly, never `Id::new_uuid().as_uuid().unwrap()`.

## egui Pitfalls

- **`Sense::click()` always includes focusable** — There is no way to make a widget clickable without also making it a Tab stop. `Sense::click()` sets the `FOCUSABLE` bit. This means collapse toggles, field labels, and insertion points are all unwanted Tab stops. A structured editor needs many clickable regions that should NOT participate in keyboard navigation. **This is the primary reason the project moved to Swift/AppKit**, where `acceptsFirstResponder` is per-view and independent of click handling.
- **Don't use `lost_focus()`** — egui's `Response::lost_focus()` is unreliable when focus moves between TextEdit widgets. It only fires if the losing widget renders *after* the gaining widget (layout-order dependent). This is a [known bug](https://github.com/emilk/egui/issues/2142) unfixed since 2022. Workaround: commit on every valid keystroke rather than on defocus. Note: Tab-driven `lost_focus` was fixed in egui 0.19-0.22; only click-to-transfer is broken.
- **`request_focus()` fights Tab navigation** — Calling `request_focus()` every frame (e.g. to keep a placeholder TextEdit focused) prevents Tab from leaving the widget. Fix: share the same egui `Id` between the inactive placeholder box and the active TextEdit, so focus transfers naturally when the widget transforms. Only call `request_focus()` on the activation frame.
- **Render pass is read-only** — The render pass takes `&Editor` and collects a `Vec<DEvent>`. All mutations happen in `handle_events` after rendering completes. This eliminates read-after-write bugs and order-dependent behavior within a frame.
- **`ui.indent` panics in horizontal layouts** — egui's `indent` only works inside vertical layouts. Since D trees can be nested arbitrarily (e.g. a `VerticalList` or `Indent` inside a `Line`), always wrap `ui.indent` calls in `ui.vertical` to guarantee a safe layout context.

## Key Design Rules

- **Documents are pure graph structure** — No semantic interpretation baked in. Use generated constants (`Field::NAME`, `Field::ISA`, etc.) for semantic access.
- **Resilient to invalid graph states** — The graph could contain anything. Projections specify the happy path but must fall through gracefully. `descend` handles missing edges (placeholder) and unexpected values (default rendering) automatically. If someone puts a C++ program in a param's name, we project a C++ program. Never assume what's at an edge; make the good case easy but don't crash or hide data on the bad case. Item_render callbacks and projections should gate with `try_wrap` and return `None` to fall through to default rendering if the type doesn't match.
- **Compile-time code generation must fail loudly** — Proc macros (`progred_macros`) generate code from the semantics graph at compile time. Unlike runtime projections, silent failures here produce subtly wrong generated code (missing fields, missing types) that compiles fine. Malformed graph data in the semantics file must produce a clear compile error, never be silently skipped.

## Architecture

- `D` enum is the render tree (`progred_core/src/d.rs`)
- `RenderCtx` builds D trees from graph data (`progred_core/src/render.rs`)
- UI renders D trees via `render_d` (`src/ui/projection.rs`)
- Events flow: UI → `DEvent` → `Editor.handle_events()`
- Selection variants: `Edge`, `ListElement`, `GraphEdge`, `GraphNode`
- `D::Descend` has `selection: Selection` — the renderer decides what selection a click produces
- `DContext` in `projection.rs` carries `path` + `selection` from nearest `Descend`
- `StackedGid` returns edges from the **first layer that has the entity**, not a merged view — if both doc and semantics have edges for the same entity UUID, only the top layer's edges are visible. This is currently correct because doc entities and semantics entities don't overlap, but would silently drop edges if they ever did. If mixed-layer entities become needed, `StackedGid::edges` would need to merge `HashMap`s.

## Migrating semantics.progred

See `docs/migration.md` for the procedure. Always use a temporary Rust binary through the project's own serde pipeline — never Python/jq/manual JSON.

## Future Considerations

- `muda` crate for native OS menus (instead of egui menus)

## TODOs

- **Domain-specific projections**: Make them more editable (currently mostly read-only)
- **Autocomplete integration**: Hook up name lookups to the placeholder autocomplete dialog, port architecture from original prototype
- **Red squiggles**: Real-time type system errors displayed inline
- **Default projection improvements**: Show placeholders for missing fields, order fields per record definition, show extra fields at bottom
- **Layout pass for block-in-inline rendering**: When a `VerticalList` appears inside a `D::Line` (e.g. default renderer: `label: [list...]`), the `[` ends up inline and elements are indented from the `[`'s cursor position, not from the logical nesting depth. Currently renders as:
  ```
  A bunch of stuff on a line [
                              Item 1
                              Item 2
                              ]
  ```
  Should render as:
  ```
  A bunch of stuff on a line [
    Item 1
    Item 2
  ]
  ```
  This can't be solved in egui's immediate mode layout without hacks — needs an intermediate layout pass (D → flat block sequence → egui) that can split a VerticalList's opening bracket onto the preceding line and place the body at the correct indent level. Domain projections (record, sum) are unaffected since their lists are inside `D::Indent`, not `D::Line`.
- **List rendering revamp**: Both list types have accumulated issues — empty horizontal list insertion slots are zero-width and invisible, vertical lists have the block-in-inline problem above, and the overall list rendering could benefit from a unified pass
- **Empty horizontal list insertion**: Empty `HorizontalList` has same discoverability problem as vertical — the insertion slot between brackets is zero-width and invisible. Share the empty-slot rendering approach from `VerticalList`
- **Unify placeholder commit events**: `PlaceholderCommitted` borrows `on_commit` from the D tree; `ListInsertCommitted` carries a path because list insertion points live in projection.rs with no D node to own a closure. Could be unified with `Rc<dyn Fn>` on `D::Placeholder`.
- **Naming audit**: "Field" vs "edge label" conflation (Field is a defined semantic thing, edge labels may or may not be fields), and related inconsistencies across D, DEvent, and UI code

## Code Style

- Very limited comments — code should be self-documenting
- Expression-oriented where possible
- Prefer long expressions broken across multiple lines over multiple statements with intermediate names — naming is hard, avoid unnecessary names
- Exception: extract helper functions when intermediate steps represent distinct semantic concepts — top-level function becomes a readable composition of named transformations (functional decomposition)
- Prefer free functions with explicit parameters over methods when `self` isn't needed — makes inputs/outputs clear, easier to unit test, enables composition in a single method that has access to `self` (Haskell-style)
- Look for generic abstractions — extract patterns in how computations combine and data flows (the way `fold`/`map`/monads abstract over structure, not specific operations)
- Dispatch on node type via `try_wrap`, not edge presence — checking "has VARIANTS edge" to mean "is a sum" is duck typing that breaks if a non-type node shares that edge label
- Apply Haskell-style thinking (explicit data flow, pure function composition) but idiomatic Rust syntax — don't fight the language
- Factor out common assignments: `x = if cond { a } else { b }` not `if cond { x = a } else { x = b }`
- Functional style: iterator chains, `try_fold`, `filter_map`, `std::array::from_fn` over mutable accumulators and loops where it doesn't make things worse
- Avoid `let mut` when a functional alternative is equally clear
- No `isX()` predicate methods — use `matches!` or pattern matching at the call site
- Eliminate partial functions: use `split_first`/`split_last` over manual indexing
- Return references for non-trivial types (let caller decide to clone), but methods on small structs (like `Document`) enable disjoint borrow checking over methods on the parent (`ProgredApp`)
- Avoid early returns — prefer `if let`, `match`, or expression-oriented alternatives over `let-else return` / `return` in closures
- Dead code should be deleted, not commented out
- UI rendering: D trees are generated from the editor state before the render pass, then the render pass walks the D tree with `&Editor` (read-only) and collects `Vec<DEvent>` — events describe what happened (user actions), not what to do about it; `handle_events` interprets them in one place after rendering
- Push back if something seems wrong
