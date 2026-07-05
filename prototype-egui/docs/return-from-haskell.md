# Return From Haskell

Date: 2026-06-25

## Status

Update 2026-07-03: superseded. The native Rust direction moved to a
fresh `prototype-linebender/` (this directory was renamed to
`prototype-egui/`), after deciding to build Puri over the Linebender
stack rather than continue in the egui shell. Kept as the record of the
decision to leave Haskell. See `../../prototype-linebender/docs/puri.md`.

As written 2026-06-25: this prototype is active again as the native Rust
direction for Progred.

The previous Rust/egui UI shell is still historical. We are not returning
to egui as the main GUI approach. We are returning to this Rust prototype
because its graph/core work remains useful, and because the next
experiment should be native Rust rather than Haskell/Wasm.

## Why The Haskell Spike Paused

The Haskell spike was justified partly by higher-kinded types and the
possibility of a Purview-style UI architecture. In practice, the work was
mostly paying for things that were not central to Progred:

- Wasm build and packaging complexity.
- JSFFI and Tauri/browser event forwarding.
- Sparse Haskell bindings for native vector graphics.
- Canvas used as a workaround for rendering, not as a desired target.
- Debugging pointer/focus behavior across a Wasm/canvas boundary.
- Interaction plumbing that was not clearly benefiting from Haskell's
  stronger abstraction tools.

The last hover/debug spike did not work reliably in Tauri and was
reverted. It is evidence that the interaction model needed a clearer
design, not code to preserve.

## Why Come Back To Rust

Rust now looks like the better place to continue because the next target
is native:

- Native windowing and renderer access are first-class.
- Vello, wgpu, tiny-skia, rust-skia, winit, and platform crates are all
  available from Rust without the Wasm/browser detour.
- The Rust GUI ecosystem is active, so a novel projection/editor approach
  has a larger nearby community.
- Rust can express enough of the useful Puri ideas with traits, builders,
  explicit data, and tests, without requiring a literal Haskell
  final-tagless architecture.
- This existing prototype already has reusable graph/model work:
  `progred_graph`, `progred_core`, `semantics.progred`, identicons, graph
  view logic, type-aware placeholders, and raw graph editing lessons.

## What Stays Historical

Keep the old history. It explains why previous directions failed.

- The egui shell remains historical because egui couples click handling
  and keyboard focus too tightly for a structured editor.
- The Swift/AppKit prototype remains useful for focus/responder-chain
  lessons, but not as the main implementation target.
- The TypeScript/Electron prototype remains useful for projection-owned
  edit behavior, tests, copy/paste, graph CLI tooling, and domain demos,
  but the DOM focus model is not the native target.
- The Haskell/Wasm prototype remains useful for layout and Puri design
  lessons, but the Wasm/canvas path is not the native target.

## New Direction

Revive this prototype around native Rust while preserving the old lessons.

Likely shape:

- Keep `progred_graph` and `progred_core` as the model foundation.
- Treat `src/ui` egui code as historical reference unless a piece is
  explicitly salvaged.
- Add a backend-independent UI/draw-command layer.
- Try Vello first, behind a renderer boundary that can be swapped for
  tiny-skia, rust-skia, or another renderer.
- Keep focus, edit state, and interaction policy as explicit app/model
  data rather than hidden toolkit state.
- Rebuild test coverage around core editing, list behavior, copy/paste,
  graph view snapshots/interactions, save/load, and schema/type matching.

The first goal is not to make a polished app. It is to prove that native
Rust can host the graph/editor model with a clearer UI runtime than the
egui, DOM, and Haskell/Wasm attempts.
