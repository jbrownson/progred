# Layout continuation cleanup

Implementation checkpoint, 2026-09-07. The direction is agreed; the remaining
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

The sibling `puri-roc` reference reinforces this: `package/Frame.roc` combines
placement output with a handler, `package/Handler.roc` composes one function over
input events, and `package/ScrollView.roc` is an ordinary widget wrapping a child
placement continuation. Its Roclay bridge inserts widgets as leaves and wraps
them with `around`, not widget-specific layout constructors. Keep Progred's
separate settled-geometry hover pass; the older prototypes are references for
composition, not replacements for that stage.

## Done in this checkpoint

- Puri handlers compose as one function over `Event`, with acceptance and an
  optional remainder. Typed registration helpers use the same composition.
  Progred's view and clipping wrappers forward this function rather than
  reconstructing seven channels. Clipping still gates starts and scroll, not
  active motion/release or keyboard/IME. Hover and rendering stages are unchanged.
- LineEdit now crosses the native continuation boundary end-to-end.
  `Layout::LineEdit` and its interpreter arm are deleted. The line widget is
  an ordinary function in the Progred editor-facing widget API; it composes
  Puri text measurement, rendering, and interaction. `Layout::Widget` carries
  an opaque measurement function returning `Measured<Fragment>`.
  The fragment contributes native render closures, a handler, hover claims,
  and a navigation transition. The editor supplies current state and scoped
  editing/selection capabilities, then incorporates these ordinary outputs.
  Styles and navigation direction are shared API types, not application imports.
  `CanvasSink` forwards native operations to any existing `Canvas`; it adds
  no GID encoding, recording, evaluator round-trip, or cross-frame state.
  The app's line adapter retains only document conversion and undo logic.
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

1. Make geometric delimiters purely geometric. Attach their hover, selection,
   and picking through explicit editor combinators with unchanged hit targets.
   Migrate other event and control wrappers through the same interface.
2. Move traversal/evaluation out of the layout interpreter. Resolve a location
   and project it during description; contribute navigation and interactions
   from placement continuations so discarded alternatives register nothing.
3. Delete obsolete Layout variants and interpreter arms as each producer moves.
   Do not retain a compatibility interpreter or replace each variant with an
   equivalent method on one giant host interface.

Completion ranking remains independently
[deferred](deferred.md#completion-ranking).

Check each slice with pure interaction/placement tests and the existing
[frame canaries](performance.md). Interactive testing is user-run.

## Verification

The affected library tests pass: 22 `measured`, 8 `progred-display`,
160 `progred-libraries`, 51 `puri`, 3 `puri-widgets`, and 261 `progred`
(seven frame profiles and one handler microbenchmark excluded).
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

The unified-handler slice was checked separately against its immediate baseline:

| Workload | Before | Unified handler |
| --- | ---: | ---: |
| IoP source | 4.19 ms | 3.70 ms |
| IoP picture | 24.68 ms | 24.57 ms |
| Fidget orbit | 10.43 ms | 9.21 ms |
| Torus | 8.43 ms | 8.07 ms |
| Tanglecube | 51.51 ms | 46.69 ms |
| Gyroid | 32.29 ms | 30.47 ms |
| Fidget cube | 15.62 ms | 14.24 ms |

No whole-frame regression appeared. A separate synthetic full traversal of
512 widgets with six event registrations each took approximately 57 µs/event,
versus 7 µs for the old separate-channel handler. A unified chain visits
unrelated event registrations too. This is a deliberate interface tradeoff,
not a dispatch speedup; the absolute cost is small against these frame workloads.
The opt-in `puri` test `handler_dispatch_profile` retains that check.

After the native LineEdit migration, the same frame medians were 3.96 ms for
IoP source, 23.56 ms for IoP picture, 8.80 ms for Fidget, 7.65 ms for Torus,
47.56 ms for Tanglecube, 30.07 ms for Gyroid, and 14.85 ms for Cube. Using the
existing in-place measurement combinators instead of temporary fragment
composition brought a repeat source run to 3.87 ms (p95 4.22 ms), against
3.70 ms (p95 4.23 ms) at the preceding checkpoint. This small interface cost
is not being presented as a performance improvement.
Generic output tests verify deferred painting, explicit hover input, navigation
and handler outputs, nested canvas clips, and discarded layout alternatives
contributing no placement output. Existing line interaction tests still cover
selection defaults, conversion, undo, caret movement, and reminted callbacks.
