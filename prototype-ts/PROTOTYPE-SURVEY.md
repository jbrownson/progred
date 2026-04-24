# Prototype Survey Follow-ups

This tracks ideas from `prototype-rust`, `prototype-swift`, and `prototype-haskell` that are worth considering for the TypeScript/Electron prototype. It also records the constraints from the follow-up discussion, especially where an idea should not be copied directly.

## Guiding Principles

- Make one consistent state change at a time. Avoid batching unrelated UI actions into an event queue unless a framework forces it.
- Prefer direct local mutations at the point where the user action is understood. Do not introduce an Elm-style action enum/interpreter unless the duplication buys something concrete.
- Undo/redo should come from snapshots or structural sharing in the graph data structure, not from replaying action records.
- Prefer using DOM/OS focus as the selected UI element when it can represent the selection directly. Add separate editor selection state only where DOM focus cannot express the desired selection.
- DOM focus is still mandatory for text boxes, so any selection model must cooperate with native focus rather than fight it.
- Raw graph editing and graph visualization are the main missing explanatory features in the TypeScript prototype.

## Priority Work

### 1. Raw Graph Mode

Goal: make the underlying graph explorable without bootstrapped semantics.

What to bring over:

- Rust's raw graph progression: unnamed UUIDs render as identicons; known `name` fields improve labels; known type/record fields enable richer projection.
- Commands/interactions to create arbitrary UUID nodes.
- Commands/interactions to connect arbitrary nodes by arbitrary labels, including labels without names.
- A way to define or designate semantics from raw graph state, rather than requiring the existing semantic layer.

Notes:

- This is the biggest missing piece in the TypeScript prototype.
- It should be possible to demo Progred from an empty or nearly empty graph.
- Raw mode will make orphan handling more important.

### 2. Graph View

Goal: show true topology alongside the tree/projection view.

What to consider:

- Rust's force-directed graph view.
- Stable deterministic initial positions based on node identity.
- Position preservation when an edge target changes.
- Edge labels rendered as names where possible, identicons otherwise.
- Node and edge selection synchronized with the projection/tree view.
- Pan, zoom, and drag interactions.

Purpose:

- Explain the real data structure visually.
- Show cycles, sharing, multiple paths, and orphan nodes.
- Support raw semantics definition without relying on an existing textual projection.

### 3. Identicons

Goal: make anonymous UUIDs recognizable without names.

What to consider:

- Rust's simple symmetric 5x5 identicon hash.
- Use identicons both in raw tree views and graph views.
- Keep the implementation DOM/SVG/canvas-native in TypeScript.

### 4. Schema Language Refresh

Goal: replace or evolve the old TypeScript `Ctor` / `AlgebraicType` schema toward the newer model.

Promising direction from Swift/Rust:

- `Record`
- `Sum`
- `Field`
- `Type Parameter`
- `Apply`
- Primitive records for strings and numbers
- Lists described in the same graph schema

Key design point:

- Type application should use type parameter node IDs as labels, not positional arguments.

Open question:

- Re-read the current TypeScript schema before designing the migration. Some ideas may already exist under different names.

### 5. Contextual Expected Types

Goal: use schema context to drive placeholders, validation, and projection.

What to bring over:

- Expected type lookup through the cursor/path.
- Substitution-aware traversal through generic `Apply` nodes.
- Tri-state matching where useful: yes, no, unknown/malformed.
- Cycle-safe type matching.

Why:

- This makes placeholder suggestions and inline mismatch display more accurate.
- It is needed for the refreshed schema language.

### 6. Placeholder and Completion Improvements

Goal: compare Rust's placeholder behavior against TypeScript's existing completion dialog and port the improvements that matter.

Rust ideas worth checking:

- Separate entry kinds: existing reference, literal, new typed node, raw new node.
- Type-compatible entries sort ahead of incompatible entries.
- Creation entries can sort ahead of references where appropriate.
- Disambiguation by type/name.
- Fuzzy tiers with deterministic ordering.
- Popup sizing based on content.

TypeScript already has:

- Existing references.
- New constructor entries.
- String and number magic entries.
- Type-aware matching.
- Fuzzy filtering.

Task:

- Do a direct comparison before rewriting. The Rust version may be more disciplined, but TypeScript already has much of the original idea.

### 7. List Insertion Affordances

Goal: make list insertion reliable and discoverable.

What to consider:

- Rust's distinct vertical and horizontal list insertion points.
- Visible empty-list insertion slots.
- Treat insertion points as actual UI focus/interaction targets where possible.

Constraint:

- Avoid inventing a complex separate selection object just to point between list nodes if DOM focus can track the active UI insertion target.

### 8. Default Projection Audit

Goal: confirm TypeScript handles malformed and partial graph states as well as Rust.

Checklist:

- Declared fields render before extra fields.
- Missing declared fields render as placeholders.
- Extra fields are still visible.
- Unexpected values fall back to default rendering instead of disappearing.
- Cycle handling remains clear.
- Field labels without names still render via identicon or raw ID.

TS likely already does part of this, but raw graph mode will stress it harder.

### 9. Orphan Handling

Goal: handle nodes disconnected from the current root once raw graph editing exists.

What to consider:

- Show orphan roots in graph view.
- Provide a way to inspect or reattach orphans.
- Consider optional garbage collection only when the user explicitly deletes/purges.

Rust has document-level reachability/orphan helpers that can guide this, but TS may need a UI-first version.

## Later Research

### Partial Invalidation

Swift's `TrackingGid` records graph reads so a graph delta can decide whether a projection needs to rerun.

This is worth revisiting later, especially for expensive projections. It is not needed for the next modernization pass.

Questions:

- Has this already been prototyped elsewhere in TypeScript?
- Should reads be tracked at edge granularity, node granularity, or projection-level dependencies?
- How does this interact with arbitrary JavaScript or TypeScript projection code?

### Layered Graph API

TypeScript already has library/document source tracking through `SourceID`, and Rust/Swift inherited similar ideas.

Potential cleanup:

- Make document, semantics/library, and primitive/builtin layers explicit.
- Preserve read-only/writeable provenance.
- Clarify whether overlapping entity IDs should merge edges or whether the top layer wins.

This is probably a cleanup of existing TypeScript ideas rather than a feature to port back.

### JavaScript and TypeScript Projection Runtime

The TypeScript prototype already has a lightweight JavaScript representation. It is worth fleshing this out.

Ideas:

- Add TypeScript as a projection authoring language.
- Use the TypeScript compiler API for checking projection code.
- Keep arbitrary JavaScript execution as the quick path.
- Decide how projection code maps errors and spans back to graph node IDs.

Haskell remains a useful reference for the hosted-language/self-hosting story, but not something to port into the TypeScript UI.

## Explicit Non-goals For Now

- Do not port Rust's event queue wholesale. It was mainly a response to egui's immediate-mode event constraints.
- Do not introduce a global action enum/interpreter just for architectural symmetry.
- Do not replace DOM/OS focus with duplicated selection state unless there is a specific selection that DOM focus cannot represent.
- Do not start with partial invalidation. Keep full rerendering until projection cost makes it necessary.
- Do not delete `graph2` as part of this thread. It remains a historical data-structure sketch and may be replaced later.

## Suggested Order

1. Add identicons.
2. Add a minimal raw graph projection.
3. Add raw graph creation/linking interactions.
4. Add or expose graph view.
5. Audit default projection under malformed/raw graphs.
6. Compare and improve placeholder behavior.
7. Revisit schema language and expected-type substitutions.
8. Expand JavaScript/TypeScript projection runtime.
9. Research partial invalidation.
