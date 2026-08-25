# Why this implementation exists

Active as of 2026-07-03. This implementation succeeded the egui and Haskell
prototypes, carrying their graph/core and Puri/Halay lessons onto the
Linebender stack. Those source trees are archived at
`archive/pre-root-promotion`.

This implementation builds Progred on Puri, a pure widget library over the
Linebender stack (winit, Vello, Parley, kurbo, peniko). Puri is the
rendering-and-behavior layer below the choice of retained, immediate,
React-style, or incremental state management: any of them can construct
ephemeral widget descriptions and place them through whatever layout policy
they choose.

Stable identity is history between evaluations, not a property an output
value can mint for itself. Snapshot reconciliation reconstructs that
provenance because it was absent from the snapshot. Puri instead asks its
caller to own any cross-frame identity and state explicitly. Consumer-owned
placement continuations solve the separate, smaller problem within one evaluation:
associating settled geometry with the behavior and other outputs of the
description that produced it.

Puri is not meant to make the smallest UI take the fewest lines. It makes
the real state and composition surface explicit so a larger UI does not
acquire a second, accidental synchronization problem as it grows.

Goals:

- **Puri.** Ephemeral widget descriptions consume caller-owned inputs
  and produce drawing, transient handlers, and other placement outputs.
  No framework state custody, minted identity, or required retained
  hierarchy. Progred's one explicit application/UI model is the simplest
  consumer, not a Puri requirement; a retained tree, identity store, or
  reconciler could produce the same descriptions. This separation also
  leaves room to experiment with incremental computation without first
  rebuilding text editing. See `docs/puri.md`.
- **Native Linebender stack.** Drawing is a final-tagless language over
  kurbo/peniko geometry and brushes. Production calls Vello or Canvas2D
  directly, while a recorder preserves inspectable, testable frames as data;
  Parley owns text. The Haskell spike proved the design but paid a
  Wasm/JSFFI/bindings tax; this stack is the same idea with the ecosystem on
  its side.
- **Editor middle-game.** Prior prototypes each reached raw editing
  plus a graph view and then pivoted. This one aims past that ceiling:
  editable domain projections, autocomplete, and a real document
  authored end to end. The data and editor model decisions are in
  `docs/model.md`.

The implementation began in a fresh directory because the egui shell used a
different UI model; salvage happened by carrying over graph/core ideas and
specific widget logic rather than refactoring that shell in place.

The Haskell spike's designs carry forward; its handoff is retained in
[`docs/history/rust-pivot.md`](docs/history/rust-pivot.md). Full historical
source and egui-era notes remain at `archive/pre-root-promotion`.
