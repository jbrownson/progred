# Prototype Survey Follow-ups

This records ideas from `prototype-egui` (then named `prototype-rust`), `prototype-swift`, and `prototype-haskell` that affected the TypeScript/Electron prototype. It is a status note, not a complete roadmap.

## Landed In TypeScript

- Raw graph editing works from an empty document: unnamed GUID nodes render with identicons, arbitrary edge labels can be created, and arbitrary existing nodes can be chosen by modifier-click from pending placeholders.
- Graph view exists as a demo/inspection pane with bubbles, arrows, panning, zooming, dragging, node/edge selection, deletion, identicons, and graph/projection selection decoration.
- Identicons are used in the default projection and graph view. Edge-label identicons use the circular treatment to distinguish them from node identicons.
- DOM focus is now the source of truth for the active editor target. The previous parallel editor-selection state was removed.
- Editor-owned commands replaced most global cursor-driven editing. Projections provide local commit/copy/key behavior through focused editor targets.
- List insertion points are focusable editor targets rather than persistent model mutations. Empty-list insertion, insertion-before-item, and comma-driven list insertion are handled in the list projection.
- Copy/paste now copies structure through projection-owned copy behavior rather than reconstructing everything from a central cursor path.
- Keyboard navigation walks the DOM/editor-target structure rather than a separate D tree.
- The React `D` compatibility layer was removed; projections now produce React components directly. The graph-defined `D` schema remains for template/render definitions in graph libraries.
- The TypeScript prototype has a substantial Vitest suite covering editor commands, focus behavior, copy/paste, list insertion, graph view helpers, rendering, save/load, and integration flows.
- The graph CLI exists:

  ```sh
  cd prototype-ts
  npm run graph -- find src/graph/libraries/type.progred "Ctor"
  npm run graph -- inspect src/graph/libraries/type.progred
  npm run graph -- render src/graph/libraries/type.progred
  ```

## Current Principles

- Make one consistent state change at a time. Avoid batching unrelated UI actions into an event queue unless a framework forces it.
- Prefer direct local mutations at the point where the user action is understood.
- Undo/redo should come from snapshots or structural sharing in the graph data structure, not from replaying action records.
- DOM/OS focus is the active editor target. Do not reintroduce duplicated selection state unless there is a selection that focus cannot represent.
- Projection-specific edit policy belongs in the projection. Lists are the main example: insertion points and comma handling are list behavior, not global cursor policy.
- The graph can be malformed. Projections should make the expected case smooth but fall through to default/raw rendering rather than hiding or crashing on unexpected data.

## Still Worth Considering

### Schema Language Refresh

The TypeScript schema still uses the older `Ctor` / `AlgebraicType` model. The Swift/Rust direction remains interesting:

- `Record`
- `Sum`
- `Field`
- `Type Parameter`
- `Apply`
- Primitive records for strings and numbers
- Lists described in the same graph schema

Key design point: type application should use type parameter node IDs as labels, not positional arguments.

This is not urgent while the current schema is sufficient for bootstrapping domains.

### Contextual Expected Types

Expected-type lookup should become less cursor/path-shaped and more projection/context-owned. Useful targets:

- Better placeholder ranking and filtering.
- Clearer mismatch rendering.
- Substitution-aware traversal through generic `Apply` if/when the schema language is refreshed.
- Cycle-safe type matching.

### Orphan Handling

Raw graph editing and graph view exposed the orphan question. Current behavior is pragmatic, not principled.

Open questions:

- Should graph view show all document GUIDs or only nodes reachable from the current root?
- Is there a semantic difference between a GUID with no edges and a GUID not present in the document map?
- Should garbage collection be explicit only?

### JavaScript and TypeScript Projection Runtime

The TypeScript prototype has a lightweight JavaScript representation and a scene/3D direction. Still useful:

- More complete JavaScript AST/schema coverage.
- Better extern modeling.
- TypeScript projection authoring eventually, likely through the TypeScript compiler or LSP.
- Error/span mapping back to graph node IDs.

### Partial Invalidation

The current React path rerenders enough for now, and recent performance issues were caused by eager completion-entry construction rather than React itself. Read tracking remains a useful research topic for expensive projections:

- Track reads at edge or node granularity.
- Invalidate only projections affected by graph edits.
- Decide how arbitrary JavaScript/TypeScript projection code participates.

This should wait until there is a real user-facing performance problem.

### Post-React Rendering

React has caused some focus/lifecycle friction, but there is not currently a strong user-facing reason to replace it. A future direct-DOM/Purview-style renderer would need:

- A concrete virtual or real DOM reconciliation model.
- A clean answer for view-local state such as collapse state and insertion-point focus.
- A better reason than architectural preference.

## Explicit Non-goals For Now

- Do not port Rust's event queue wholesale. It was mainly a response to egui's immediate-mode event constraints.
- Do not introduce a global action enum/interpreter just for architectural symmetry.
- Do not duplicate DOM focus with editor selection state.
- Do not start with partial invalidation until projection cost makes it necessary.
- Do not let the demo graph view drive core model complexity.
