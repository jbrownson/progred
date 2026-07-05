# Why this prototype exists

Active as of 2026-07-03. Successor to `prototype-egui/` (formerly
`prototype-rust/`), whose brief reactivation after the Haskell pause
concluded that the graph/core work is worth keeping but egui is not the
UI direction.

This prototype builds Progred on Puri, a pure widget library over the
Linebender stack (winit, Vello, Parley, kurbo, peniko).

Goals:

- **Puri.** Widgets as pure functions from (persistent widget state,
  props) to (draw calls, handlers). No framework state custody, no
  minted identity, no retained hierarchy — the widget tree is a
  function of the app model every frame. State management is
  deliberately out of scope so it can be experimented with separately
  from widget behavior. See `docs/puri.md`.
- **Native Linebender stack.** The draw list is expressed in
  kurbo/peniko types and is itself the inspectable, testable value;
  Vello renders it behind a boundary; Parley owns text. The Haskell
  spike proved the design but paid a Wasm/JSFFI/bindings tax; this
  stack is the same idea with the ecosystem on its side.
- **Editor middle-game.** Prior prototypes each reached raw editing
  plus a graph view and then pivoted. This one aims past that ceiling:
  editable domain projections, autocomplete, and a real document
  authored end to end. The data and editor model decisions are in
  `docs/model.md`.

Why a fresh directory: the egui shell is a different UI model kept as
reference; salvage is by copying (graph/core crates, specific widget
logic from Masonry with attribution), not refactoring in place.

The Haskell spike's designs carry forward; the handoff is recorded in
`../prototype-haskell/RUST_PIVOT.md` and the egui-era lessons in
`../prototype-egui/AGENTS.md`.
