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

- Native preparation keeps document-site state and editing capabilities behind
  an explicit request. Inert widgets and pointer/hover wrappers do not resolve
  a path or construct text-editing callbacks. Prepared text controls, delimiters,
  and empty outlines retain whole-widget render callbacks; `Fragment` no longer
  implements Canvas by recording one closure per operation.
- Hover claims, occlusion, and hover feedback are ordinary native decorators.
  `OnHover` is removed. Puri owns identity-parametric settled hover probes; the
  editor adds owning-view scope without duplicating the probe policy. Insert
  and collapse handles explicitly request their wash rather than letting the
  interpreter infer it from their target type. `EmptySlot` is removed too;
  `slot` and pending views use the same native empty-outline widget.
- Click, activation, and picking are ordinary native interaction functions,
  not Layout variants or app interpreter cases. One generic `widget::before`
  prepares a placement callback returning the same native outputs as a leaf;
  it can wrap any child without inspecting or converting that child's widgets.
  The callback runs only when the chosen child places, before the child's
  outputs, preserving child-first event acceptance. Explicit pick values use
  the caller-supplied pick capability, rather than an editor-interpreted opcode.
- Delimiters are ordinary native side widgets. `Surround` now owns only the
  baseline composition and the sides' width budgets; opaque callbacks measure
  the two sides against the chosen child's span. The interpreter no longer
  knows delimiter ink, document targets, or selection/picking rules.
  `bracket` is inert, and `selectable_bracket` composes the generic `selectable`
  measured-widget decorator through `selectable_side`. Existing structural
  and expression projections opt in explicitly. The low-level Grap bracket
  constructor is intentionally inert; no checked-in example relied on its
  previous implicit selection. Native handlers now receive settled hover at
  dispatch, respecting owning views and the existing pointer propagation order.
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

1. Migrate Grap-event, gesture, and remaining control wrappers through
   the native widget interface. Completion and its navigation/offer outputs
   remain host requests.
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

The affected library tests pass: 22 `measured`, 14 `progred-display`,
160 `progred-libraries`, 52 `puri`, 3 `puri-widgets`, and 266 `progred`
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

The delimiter slice adds tests for inert/selectable ink and extent equivalence,
bounded width growth, retained-hover activation/picking, owning-view isolation,
clipped sides contributing nothing, and arbitrary side widgets seeing only the
chosen alternative's span. Existing cell-interior, padded-handle, and editable
path tests pass. IoP source measured 4.12 ms (p95 4.67 ms), against the preceding
3.87 ms checkpoint; the final serial canary run measured 3.90 ms (p95 4.09 ms).
This is within the variation of these short runs, not a claimed speedup. The
same serial run measured IoP picture 23.58 ms, Fidget 10.79 ms, Torus 6.67 ms,
Tanglecube 48.38 ms, Gyroid 34.01 ms, and Cube 15.11 ms. GPU-oriented canaries
vary more; they are not a controlled attribution of cost to this interface.
No cache or layout-search change was introduced. Native/web checks and format
checks pass, with only the previously noted web menu warnings.

The native pointer-action slice adds regressions for live-world callbacks,
clipped raw clicks, retained-hover activation/picking, child-first acceptance,
and arbitrary leading continuations placing only in the chosen alternative.
Existing gesture tests now compose the native action callbacks, preserving
pending-pick precedence, occlusion, and view ownership. The serial frame run
measured IoP source 4.05 ms (p95 4.20 ms), IoP picture 22.83 ms, Fidget 10.01 ms,
Torus 6.81 ms, Tanglecube 45.82 ms, Gyroid 31.71 ms, and Cube 13.91 ms. Source
is slightly above the preceding 3.90 ms checkpoint; this interface change is
not a performance optimization. No caching or event-relevance rules were added.

The native hover/empty-outline and render-granularity slice measured IoP source
4.15 ms (p95 4.32 ms), against 4.05 ms (p95 4.20 ms). The other medians were
IoP picture 22.56 ms, Fidget 9.28 ms, Torus 7.14 ms, Tanglecube 46.64 ms, Gyroid
30.84 ms, and Cube 14.59 ms. Tests cover per-widget deferred drawing, inert
wrappers never requesting site state, independent hover claims and feedback,
mapped retaining/exact/dynamic/occluding probes, clipping, and releases passing
occluders. These are regression checks, not a claimed speedup.
