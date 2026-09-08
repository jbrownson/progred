# Layout and widget continuations

Current architecture, 2026-09-07.

## Boundary

Layout places boxes with opaque continuations. Widgets written in the Puri
style need not live in the Puri package: reusable controls belong in
`puri-widgets`, while document-aware widgets and adapters belong in Progred.
Projection, navigation, selection, hover policy, and event interpretation are
not box primitives.

The reference is tag `archive/pre-root-promotion`:

- `prototype-haskell/halay/src/Halay.hs`: `Measured` pairs size with
  `Placement -> placeM placed`; leaves and decorators contribute continuations.
- `prototype-haskell/puri/src/Puri/Widget.hs`: a widget takes settled placement
  and returns rendering plus handlers.
- `prototype-haskell/progred/src/Progred/Projection.hs`: `descend` resolves a
  location and calls the projection, rather than emitting a layout opcode.

The sibling `puri-roc` reference reinforces this: `package/Frame.roc` combines
placement output with a handler, `package/Handler.roc` composes one function over
input events, and `package/ScrollView.roc` wraps a child placement continuation.
Its Roclay bridge inserts widgets as leaves and wraps them with `around`.

We retain Progred's baseline box algebra and ordered alternatives, not Clay,
and preserve its separate settled-geometry hover stage.

## Frame construction

1. Partials produce `Layout<World, Hover>` programs, above the total structural
   fallback. Each program calls the layout builder interface; it is not an enum
   description. Native functions supply controls and projection recursion.
2. The production builder prepares one `ChoiceLayout<HoverPass<World, Hover>>`
   graph. Resolution chooses alternatives and settles the selected geometry in
   that graph. Its root is exposed as `Measured<HoverPass<World, Hover>>`: an
   extent and a one-shot placement function, not another container tree.
3. Placement calls only the chosen continuations with their full rectangle and
   effective enclosing clip. They return an opaque hover continuation; no hover
   query, painting, or event dispatch runs during placement.
4. The app calls that continuation with the pointer and retention inputs.
   It raises floaters and runs widgets topmost-first. Each widget answers hover
   against its settled geometry and contributes paint continuations, handlers,
   and navigation data. The resulting `Fragment` holds the winning claim, not a
   retained list of probes. Lower widgets still produce their paint and handlers
   after a direct claim wins; only their hover queries are occluded.
5. The app resolves the claim and may run the paint continuations, supplying the
   settled hover. They draw directly into the chosen canvas backend. Handlers
   and navigation are separate outputs: dispatch never requires painting first.

The central sequence is:

```rust,ignore
let view = app_view(description, resources);
let hover = measured::place(view, placement);
let ready = hover.run(&hover_input);
// Resolve ready.claim, then paint ready.renders if requested.
// Retain ready.handler and navigation for later input dispatch.
```

Navigation stops are declared by projection/widget functions; placement only
supplies their rectangles. A control's arrival override is consumed by its
nearest navigation landmark. Discarded alternatives contribute no navigation,
hover, handlers, or painting.

## Ownership and types

[`Builder`](../display/src/builder.rs) accepts box composition and opaque
leaf/preparation functions. Rows, columns, overlays, padding, floating, sharing,
and alternatives own geometry. `before` and `after` compose placement work
without interpreting it. Floating receives a positioning function and does not
install popover policy.

[`display/src/measure.rs`](../display/src/measure.rs) implements this interface
with the choice graph. Its `Node` results are temporary interpreter-local slots,
consumed by parent operations; they are not persistent identities or paths.
Shared children use an explicit sharing key and prepare only once per frame.
The selected graph invokes its leaves directly. Rows, columns, and padding use
the same geometry helpers as the plain `Measured` combinators; there is no
`Measured::Kind` interpreter. Opaque wrappers receive a child's extent and
placement callback when choices have settled.

[`recording`](../display/src/recording.rs) is a test-only implementation of the
same calls. It retains structure for inspection without executing opaque widget
programs. Production has no parallel `Layout` enum. Layout programs are reusable
closures, however, so their captures and the choice graph still allocate; this
is not an allocation-free or entirely streaming layout engine.

`Placement` carries a separate `available_rect` alongside the actual rectangle
and enclosing clip. Rows offer their vertical span to children without changing
their natural rectangles; columns offer their width, overlays their full bounds,
and padding insets the available rectangle. `fill_height` is an ordinary
placement wrapper which adopts that span. Brackets are fixed-width widgets in a
row, with this wrapper around each side; there is no `Surround` operation or
width/height feedback. Leaf debug outlines use the final adopted rectangle.

A `Widget` prepares a `Measured<HoverPass>`. A `Program` can instead prepare
a subtree with choices, using the same per-frame `ChoiceBuild`. Neither is a
catalogue of control variants. Puri's plain text/drawing leaves are measured by
[`widget::drawing`](../display/src/widget/drawing.rs); they contain no document
or interaction information.

[`HoverPass`](../display/src/widget/frame.rs) is the shared placement output;
the app's `Placed` aliases it. It composes one-shot functions, using a flat
sequence for siblings rather than a recursive call stack. `run` returns a
`Fragment` (the app's `Ready`): the hover claim, rendering, one
function-over-`Event` handler chain, navigation, view regions, and exact
completion offers. `HoverContext` is a transient output builder inside a
widget's hover callback. Its contribution methods do not expose neighboring
widgets' buffers. The pointer input is not retained in paint or handlers;
those receive the resolved target later.

Hover composition streams into shared output buffers. Hover visits front to
back; the paint and landmark segments are reordered to preserve their ordinary
back-to-front construction order. Scoped wrappers map a child's completed
output, consuming navigation overrides without affecting siblings or ancestors.
Floaters are lifted before hover and remain outside the enclosing clip/navigation
scope. These editor-facing types belong to `progred-display`; Puri itself remains
layout-neutral and knows no document paths.

[`CanvasSink`](../ui/puri/src/draw.rs) is the object-safe primitive drawing
interface implemented by Vello, Canvas2D, and the test recorder. `Canvas`
supplies generic convenience methods above it. Whole-widget render closures
call that interface directly; there is no backend-specific fragment conversion
or new GID drawing list. Canvas clips use scoped boxed callbacks. Existing
initial encodings remain available where recording is intentional.

## Projection recursion and controls

`descend`, `at`, and `transient` build ordinary preparation functions using
an explicit [projection scope](../display/src/widget/project.rs). The app
supplies source lookup, cycle detection, provenance, fuel, and the selected
current/descendant partials. There are no corresponding Layout enum cases,
and the editor no longer pattern-matches on Layout.

Preparation still interleaves projection and measurement. These functions run
before choice resolution and placement, not eagerly while a partial first
constructs its description. That retains sharing: a descendant referenced by
several alternatives prepares once in that frame. The next frame starts a fresh
builder. No cross-frame cache or additional layout search was introduced.

`current` and `default` remain separate explicit projection inputs. Missing
locations pass `None` to the chosen partial and fall back to the ordinary
pending widget; computed roots remain read-only. The GID layout library's
path-bearing forms decode through the common path library into these functions,
not a second path opcode interpreter.

Grap can still return stored GID layout forms for decoding, or produce an
explicit `layout program`. The latter holds an ordinary Grap closure; its
partial applies that closure with borrowed FFIs closed over a Rust output
buffer. Box and widget calls emit native layouts into that buffer, never native
objects or handles into Grap. Grouping calls evaluate a raw body while collecting
its children, then emit their parent. The buffer is evaluation-local, and a halt
or invalid builder call discards it. See [the scoped interface](../libraries/src/layout/scope.rs).

The effect boundary is explicit so existing value-returning projection
combinators keep their semantics. For example, `border` can wrap a returned
layout program just as it wraps another projected value. It does not inspect
or intercept the builder's effects. Native widgets do not take either GID path,
and neither path encodes every drawing operation as GID before painting.

Line editing, delimiters, pointer actions, hover feedback, scrubbing, state
scrolling, borders, and popovers are native widget functions/combinators.
Completion and drawing-program functions request scoped app adapters during
preparation; the adapters own document-specific offers/evaluation/source
attribution and return the same measured widget output. They are not operations
of the box interpreter. Completion providers remain explicit lazy inputs and
run only for the active picker.

Native widgets request site state and editing capabilities only when needed.
Inert decorations do not construct text state or Grap interpreters. Conversion
callbacks belong to the current line handler, never persistent selection.
Event acceptance is independent of whether state changed; scroll handlers return
unused displacement explicitly. Scroll, clipping, floating, and navigation
composition are shared rather than duplicated between native and editor output.

## Verification

Regression tests cover contextual projection scopes, missing locations,
computed roots, source-qualified follows, completion, editing/conversion,
gesture/view ownership, clipping, navigation, floaters, and shared preparation.
Projection-shape tests record both layout calls and projection recursion;
neither recording representation is used by production.

The frame canaries in [performance.md](performance.md) exercise complete
headless frames, including disposal. They are regression checks, not interactive
frame-rate measurements. Completion ranking remains independently
[deferred](deferred.md#completion-ranking).

Phase-specific tests verify that placement does not run hover, hover does not
paint or dispatch, paint sees the final target, and discarding paint leaves a
working handler. Ordering tests cover multiple paint operations per widget,
occluded queries, retention, view ownership, and floating overlays. Debug hover
footprints are collected only when requested and do not change the claim.
No search policy or Fidget rendering algorithm changed. Unified event handlers
still visit unrelated registrations; the opt-in `handler_dispatch_profile`
retains a focused check of that interface cost.

The continuation and geometry passes had mixed performance results, including
regressions. They are not justified as speed improvements. The consolidated
builder's phase timings and sampling findings are recorded in
[performance.md](performance.md#layout-builder-and-placement-consolidation--2026-09-07).

The consolidated version passes all 612 workspace library unit tests (eight
opt-in tests excluded), all seven release frame canaries, the native
workspace/all-target check, and the wasm32 library check. The web check retains
two existing menu dead-code warnings. Sample, Grap, and cube headless SVGs were
visually inspected; the application was not launched.

The subsequent scoped-FFI pass passed the workspace library tests, all eight
release canaries (including the new interleaved A/B test), native all-target
checking, and wasm32 checking with those same existing warnings. Its full-frame
test compares native emission with GID construction/decoding, including the
unchanged border combinator. Scope tests cover lexical capture, nested child
buffers, invalid inputs, fuel exhaustion, and non-escaping capabilities. The
Grap evaluator itself remains unchanged.
