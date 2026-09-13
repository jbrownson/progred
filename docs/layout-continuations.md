# Layout and widget continuations

Current architecture, 2026-09-08.

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

## Event and frame cycle

[`input.rs`](../progred/src/input.rs) interprets the existing keyboard, pointer,
and IME events. Its `update_frame` calls run the installed frame's handler before
building its successor. The shell translates platform input and schedules painting;
it no longer contains the text/structural keyboard chain or pointer selection rules.
`update_frame` takes an ordinary event-interpreting function, not another event
encoding or an action queue. Dispatch is always present: its default is an empty
handler and navigation data. Window setup builds the initial frame once the window
size is known. Document replacement clears old dispatch to that no-op and builds
the successor immediately when a window is active. Event handling has no frame
initialization branch. Unhandled events with unchanged frame inputs retain the
installed frame.

Each window owns an `EditorRunner`: mutable `Editor` state beside `FrameState`
(dispatch, settled hover, and optional pending painting). The runner also owns
queued continuous input. Widget handlers still receive only `&mut Editor`;
the saved handler is never part of the state passed back to itself. `update_frame`
borrows the editor mutably and the previous dispatch/hover immutably, then installs
the successor after dispatch returns. There is no temporary no-op substitution,
handler clone, or restore step. The no-op is only the initial/reset frame.

Document replacement has the same separation: the editor finalizes gestures and
replaces document-owned state; the runner clears old frame outputs and queued
input, then prepares the successor. No replacement flag or snapshot comparison
is needed to discover that the frame was invalidated.

The successor is built by `refresh_frame` in
[`frame.rs`](../progred/src/frame.rs):

1. Partials produce `Layout<World, Hover>` programs, above the total structural
   fallback. Each program calls the layout builder interface; it is not an enum
   description. Native functions supply controls and projection recursion.
2. The production builder prepares one `ChoiceLayout<HoverPass<World, Hover>>`
   graph. Resolution chooses alternatives and settles the selected geometry in
   that graph. Its root is exposed as `Measured<HoverPass<World, Hover>>`: an
   extent and a one-shot placement function, not another container tree.
3. Placement calls only the chosen continuations with their full rectangle and
   effective enclosing clip. They run hover probes immediately, in painting
   order, against the current pointer and retention inputs. No painting or event
   dispatch runs here. Later direct hits and occluders supersede earlier claims;
   an extended claim only retains a target when there is no stronger claim.
4. Floating placements run after ordinary content. A floater's nested floaters
   run before the next sibling floater. The resulting `HoverOutput` contains the
   winning claim, navigation declarations, and `AfterHover` continuations, not
   a retained list of ordinary hover callbacks or probes.
5. The app constructs `ResolvedHover` and binds the continuations. This produces
   rendering and handlers independently. Each render is now a canvas-only
   callback capturing the resolved input from this frame. Presenting it cannot
   read a newer hover or debug setting. Painting remains optional.

The overall sequence is:

```rust,ignore
// Input: run the previous handler, then project, place, and resolve hover.
runner.update_frame(scale, viewport, handle_event);

// The platform's later paint request consumes the latest completed frame.
puri::frame::render(runner.prepare_paint(scale, viewport).renders, canvas);
// After successful submission, schedule another redraw if hover has a follow-up.
let redraw = runner.frame_presented();
```

Navigation stops are declared by projection/widget functions; placement only
supplies their rectangles. A control's arrival override is consumed by its
nearest navigation landmark. Discarded alternatives contribute no navigation,
hover, handlers, or painting.

`compute_hover` owns the whole hover stage: probing, preserving the prior target
during a press, attributing the winner, and binding the continuations. It returns
a completed `Frame`; it never updates the runner's stored hover.

Selection reveal is part of handling input, not frame construction. `update_frame`
compares the selection's view/path/stage before and after the action. When it
changes, the installed frame's navigation rectangle determines a one-shot scroll
adjustment before building the successor. Native menu commands use that same
boundary. Nothing remembers which selection was revealed: plain refreshes and
manual scrolling do not request another reveal. A path missing from the installed
frame leaves the offset unchanged, with no deferred retry. Substantially reflowed
destinations are deliberately best-effort. There is no reveal flag, persistent
target mode, corrective build, or new layout/hover boundary for selection reveal.

After installing a different hover target or owning view, the runner dispatches
`Event::HoverChanged` through that frame's ordinary handler chain. The dispatch
context supplies the settled target and navigation/view geometry. An accepted
notification builds one successor, regardless of whether the handler wrote any
state. There is no recursive dispatch or attempt to infer which inputs a widget
depends on.

Further hover reactions wait until that successor has been painted and submitted.
`frame_presented` releases the boundary and requests another redraw when the
current hover still differs from the last notification. Preparing or discarding
paint, and failed native presentation attempts, do not release it. A handler
which keeps changing hover therefore advances across painted frames, yielding to
the event loop between them, without a cycle panic or iteration cutoff. An
accepted no-op stops because the successor has the same hover.

Notifications describe the current settled hover, not a history of discarded
frames. As with other staged frames, intervening input may replace an unpainted
successor; notification then uses the latest target and its current handler,
never a saved callback into the replaced document. Document replacement resets
notification state along with the rest of the frame.

The drawing widget handles hover changes by explicitly revealing the winning
source in the navigation geometry. Its ordinary `ModifiersChanged` handler
does the same when Cmd is pressed over a stationary pointer. Both require the
link modifier, the owning view, and a pointer inside the drawing's clipped
placement. The frame runner contains no canvas-specific reveal operation.

Two ordering boundaries remain explicit. Touch establishes a hover target before
its first press because it has no preceding pointer motion. Continuous input is
batched: scroll runs and settles geometry before the paired motion handler runs.
That scroll frame already includes the latest pointer position, so unhandled
paired motion needs no second frame. Motion on its own still rebuilds hover.
Handled motion produces a successor as usual. The batching does not reuse a
frame after its inputs change; it avoids projecting the same settled inputs twice.

Painting is separate because input and redraw requests have different schedules.
Several input transitions may replace unpainted frames, but their handlers still
run in order. `prepare_paint` consumes the staged painting, or builds a frame when
the viewport changed or no staged frame remains. Render callbacks run only there;
updating a frame never calls the canvas.

## Ownership and types

[`Builder`](../progred/src/display/builder.rs) accepts box composition and opaque
leaf/preparation functions. Rows, columns, overlays, padding, floating, sharing,
and alternatives own geometry. `before` and `after` compose placement work
without interpreting it. Floating receives a positioning function and does not
install popover policy.

[`display::measure`](../progred/src/display/measure.rs) implements this interface
with the choice graph. Its `Node` results are temporary interpreter-local slots,
consumed by parent operations; they are not persistent identities or paths.
Shared children use an explicit sharing key and prepare only once per frame.
The selected graph invokes its leaves directly. Rows, columns, and padding use
the same geometry helpers as the plain `Measured` combinators; there is no
`Measured::Kind` interpreter. Opaque wrappers receive a child's extent and
placement callback when choices have settled.

[`recording`](../progred/src/display/recording.rs) is a test-only implementation of the
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
[`widget::drawing`](../progred/src/display/widget/drawing.rs); they contain no document
or interaction information.

[`HoverPass`](../progred/src/display/widget/frame.rs) is the running consumer of
settled placements. `HoverContext` is the transient interface used by a widget:
answer probes, declare navigation, and supply `after_hover` continuations.
`render` is a combinator over that boundary; native handlers that do not need
the winner are lifted into it at the end of the widget's contribution. Handlers
that need the winner can instead be constructed inside `after_hover`.
`finish` runs floating placements and returns `HoverOutput`. Its consuming
`bind` operation uses `ResolvedHover` to assemble `Effects`, then returns a
distinct `FrameOutput`: canvas-only renders, one function-over-`Event` handler
chain, and settled navigation/view geometry. A hover output cannot be bound
twice or masquerade as a completed frame.

[`puri::frame::AfterHover<H, O>`](../ui/puri/src/frame.rs) owns the reusable
phase-composition mechanism. Both the resolved input and output are generic;
there is no dependency on Progred, its identities, or `measured`. It is optional
plumbing for widgets, not a required Puri layout or widget protocol. Puri also
owns probes and claim precedence. Progred owns source attribution, view identity,
navigation, and popup policy.

Hover and continuation collection run in painting order. There are no reversed
paint/navigation segments. Scoped wrappers map a child's output, consuming
navigation overrides without affecting siblings or ancestors. Floating placements
escape enclosing clip/navigation scopes but retain their owning view.

Progred's `attribute_hover` function derives secondary identity and source trace
once for the winner. `ResolvedHover` explicitly shares those results within this
frame, avoiding repeated source-path walks by each painted occurrence. It is
freshly constructed, not retained as a cross-frame memo. Debug geometry is
editor configuration captured while constructing the frame, not hover data.

[`CanvasSink`](../ui/puri/src/draw.rs) is the object-safe primitive drawing
interface implemented by Vello, Canvas2D, and the test recorder. `Canvas`
supplies generic convenience methods above it. Whole-widget render closures
call that interface directly; there is no backend-specific fragment conversion
or new GID drawing list. Canvas clips use scoped boxed callbacks. Existing
initial encodings remain available where recording is intentional.

## Projection recursion and controls

`descend`, `at`, and `transient` build ordinary preparation functions using
an explicit [projection scope](../progred/src/display/widget/project.rs). The app
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
or invalid builder call discards it. See [the scoped interface](../progred/src/libraries/layout/scope.rs).

The effect boundary is explicit so existing value-returning projection
combinators keep their semantics. For example, `border` can wrap a returned
layout program just as it wraps another projected value. It does not inspect
or intercept the builder's effects. Native widgets do not take either GID path,
and neither path encodes every drawing operation as GID before painting.

Line editing, delimiters, pointer actions, hover feedback, scrubbing, state
scrolling, borders, and popovers are native widget functions/combinators.
Completion and drawing-program functions call app adapters during
preparation; the adapters own document-specific offers/evaluation/source
attribution and return the same measured widget output. They are not operations
of the box interpreter. Completion providers remain explicit lazy inputs and
run only for the active picker.

Progred widgets borrow site state during preparation. Their transient handlers
receive the concrete editor at dispatch and call ordinary editing helpers;
there is no per-site dictionary of callback factories. Puri remains independent
of the editor. Inert decorations do not construct text state or Grap interpreters. Conversion
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
