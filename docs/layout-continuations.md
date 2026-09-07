# Layout continuation cleanup

Implementation checkpoint, 2026-09-06. The direction is agreed; the remaining
steps below are not yet implemented.

## Boundary

Layout should place boxes with opaque continuations. Widgets written in the
Puri style need not live in the Puri package: reusable controls belong in
`puri-widgets`, while document-aware adapters belong in Progred. Projection,
navigation, selection, hover, and event interpretation are not box primitives.

The reference is tag `archive/pre-root-promotion`:

- `prototype-haskell/halay/src/Halay.hs`: `Measured` pairs size with
  `Placement -> placeM placed`; leaves and decorators contribute continuations.
- `prototype-haskell/puri/src/Puri/Widget.hs`: a widget takes settled placement
  and returns rendering plus handlers.
- `prototype-haskell/progred/src/Progred/Projection.hs`: `descend` resolves a
  location and calls the projection, rather than emitting a layout opcode.

Recover that boundary, not the Clay algorithm. Preserve the current baseline
boxes, ordered alternatives, within-frame sharing, and explicit projection
scopes. Do not reintroduce an implicit ambient projection or cross-frame cache.

## Done in this checkpoint

- Layout's GID traversal descriptions now use the path library, including list
  elements and source-qualified follows. The duplicate encoding is removed.
- State-scroll handlers use the same partial-consumption `ScrollOutcome` as
  ordinary scroll containers, independent of whether they update state. The
  adapter converts the remainder back to the incoming units. Fidget passes
  horizontal scrolling and any displacement beyond its zoom limits outward.
- Ordered choice resolution lives in `measured::choices`, generic over placement
  output. Its builder owns slot bookkeeping; popover policy remains outside it.
  Placement tests cover chosen alternatives, shared children, clipping,
  wrapper order, and optional out-of-flow placement.
- Line conversion belongs to the current handler, not persistent selection.
  Native text, number, blob, and color controls call native conversions; the
  line library adapts Grap conversions explicitly. The document-aware line
  control owns conversion and Puri composition. Puri accepts a caller-run
  editing operation, so document writes and undo grouping happen at that
  boundary, not in a shell-wide post-event step. Query editing also resets
  completion state there. Caret-only operations do not invoke conversion.

## Remaining migration

1. Move LineEdit through the continuation boundary end-to-end. Keep the actual
   Puri editor; remove its special request from the upper Layout language.
   Handler-owned conversion and native callbacks are done; `Layout::LineEdit`
   is still a host request and must be removed. Preserve undo
   grouping, intermediate spellings, IME, clipboard, and default caret behavior.
2. Make geometric delimiters purely geometric. Attach their hover, selection,
   and picking through explicit editor combinators with unchanged hit targets.
   Migrate other event and control wrappers through the same interface.
3. Move traversal/evaluation out of the layout interpreter. Resolve a location
   and project it during description; contribute navigation and interactions
   from placement continuations so discarded alternatives register nothing.
4. Delete obsolete Layout variants and interpreter arms as each producer moves.
   Do not retain a compatibility interpreter or replace each variant with an
   equivalent method on one giant host interface.

Completion ranking remains independently
[deferred](deferred.md#completion-ranking).

Check each slice with pure interaction/placement tests and the existing
[frame canaries](performance.md). Interactive testing is user-run.

## Verification

The affected library tests pass: 22 `measured`, 5 `progred-display`,
160 `progred-libraries`, 47 `puri`, and 261 `progred` (seven opt-in profiles excluded).
Scroll regressions cover acceptance without writes, pixel/line/page unit
round-tripping, and unused input at the camera's zoom limits.
Line-control regressions cover a different conversion after reminting, no
conversion on caret movement, and native/Grap conversion equivalence for text,
blob, f32, f64, u64, and color, including invalid spellings and extra metadata.
The wasm32 check passes with unused-menu warnings for `drawn_menu` and `Quit`
in unchanged code. Formatting and whitespace checks pass.
Those seven profiles also pass serially in release mode. Before/after the
choice-engine extraction, with five warm-up and 60 measured frames:

| Workload | Before | Choice extraction | Handler-owned lines |
| --- | ---: | ---: | ---: |
| IoP source | 4.10 ms | 4.06 ms | 3.91 ms |
| IoP picture | 25.73 ms | 23.09 ms | 23.77 ms |
| Fidget orbit | 10.43 ms | 10.28 ms | 9.90 ms |
| Torus | 8.34 ms | 8.18 ms | 7.92 ms |
| Tanglecube | 50.80 ms | 47.09 ms | 47.18 ms |
| Gyroid | 31.65 ms | 30.52 ms | 30.70 ms |
| Fidget cube | 16.14 ms | 15.71 ms | 15.46 ms |

This is a regression check, not evidence of a speedup from moving code. The
selection algorithm is unchanged and the short runs have ordinary timing
variation. These are headless frame-to-DrawList measurements under the build
sandbox, not GPU presentation or interactive frame-rate measurements.
