# Puri and the editor frame

Current implementation, 2026-09-04. This describes the code and its boundaries;
it does not attribute the earlier framework comparisons or future plans to the
owner. Those notes are retained in [history](history/puri-notes.md).

## Ownership

Puri provides ephemeral widget descriptions, drawing, interaction helpers,
and caller-owned state types. A description receives state and presentation
inputs, then uses settled geometry to draw and register handlers. Puri retains
no application hierarchy, mints no identity, and owns no global clipboard,
window, clock, or focus service.

The caller owns focus and durable interaction state. `LineEditState` contains
text and cross-frame editing state; `LineEditDescription` supplies font, paint,
affixes, focus, placeholder, and chrome for this description. `EditCtx` supplies
mutable state, Parley contexts, and a clipboard capability at dispatch. The
focused editor emits a caret rectangle for the platform IME.

The caller runs each `EditOperation` with that `EditCtx` rather than lending
the editor out to a handler. This lets the caller finish the state/service
borrows, then apply a document conversion and record undo as one interaction.
Puri does not know about those document operations. Progred captures conversion
in the current line handler, never in persistent selection state.

At Progred's line-control boundary, missing editing state has a defined default:
the current spelling with the caret at its end. The frame uses that state without
persisting it; dispatch materializes it on access for an editing interaction.
Selection operations need not eagerly construct widget state. This policy belongs
to the line control, not to generic selection or the data model.

The package boundaries are:

| Package | Responsibility |
| --- | --- |
| [puri](../ui/puri/src/lib.rs) | Canvas and text vocabulary, placement geometry, typed handlers, pure widget descriptions |
| [puri-widgets](../ui/puri-widgets/src/lib.rs) | Reusable composed widgets, including completion rows |
| [measured](../ui/measured/src/lib.rs) | Box composition and ordered alternatives with opaque placement outputs |
| [uig](../ui/uig/src/lib.rs) | Shared geometry vocabulary (`Placement`), re-exported by Puri |
| [puri-vello](../ui/puri-vello/src/lib.rs) | Native Vello canvas backend |
| [puri-web](../ui/puri-web/src/lib.rs) | Browser Canvas2D backend |
| [Progred placement](../progred/src/placed.rs) | Editor hover, navigation, popup policy, deferred paint, and dispatch inputs |

Reusable widgets do not interpret document values or choose domain completion
vocabulary. Progred and its projection libraries supply those choices.

## Measurement and placement

Puri owns `Placement { rect, available_rect, clip_rect }`, not a layout algebra.
`rect` is the widget's rectangle. `available_rect` is an optional expansion
offered by its container: rows offer their vertical span, columns their width,
and overlays both. Ordinary widgets ignore it. A `fill_height` combinator adopts
the available vertical span before invoking the child's placement continuation.
This does not change intrinsic measurement or trigger another choice search.
`clip_rect` is the effective enclosing axis-aligned clip,
not already intersected with the widget. Ordinary children inherit the clip;
clipping containers intersect their bounds into it. Hover and gesture starts
must lie inside both rectangles. Motion and release for an active gesture can
continue outside them.

Canvas clips are separate: they constrain ink and can use arbitrary shapes.
An axis-aligned layout clip neither describes nor replaces an arbitrary canvas
clip. Clipping does not in itself remove navigation or active handlers.

Progred composes boxes by width, ascent, and descent. Rows align baselines or
centers; columns choose a baseline; wrappers pad, overlay, or decorate the
result. [`measured::choices`](../ui/measured/src/choices.rs) settles
ordered alternatives over already measured leaves. The first preferred form
whose natural width fits wins; otherwise the last form accommodates the
available width. Selection does not reshape text or rerun projections.
Shared layout nodes belong to this one frame and are consumed by the selected
form.

The choice engine has no GID, editor, or Puri dependency. `ChoiceBuild` owns
per-frame sharing and choice bookkeeping; `resolve_choices` returns a
`Measured<Out>`: an extent and placement callback. The selected choice graph
places directly, without building a second measured container tree. Plain
measured combinators compose callbacks using the same geometry helpers, not a
`Kind` enum. Wrappers do not interpret their output. Out-of-flow
content uses `attach`: only the base contributes to surrounding width, and
the consumer supplies how the two settled subtrees place. Popover styling,
position, occlusion, and raising remain Progred policy.
The display builder's `floating` operation supplies only two boxes and a
positioning function. The ordinary [popover widget](../progred/src/display/widget/popover.rs)
composes its padding, panel ink, and input blocking explicitly; the layout
interpreter does not add these to a floating box.

Native widgets use `progred::display::widget::Widget`: a measurement function
whose result places a `HoverPass`. Calling that continuation produces a
`Fragment` of deferred ink, handlers, the hover claim, and navigation declarations.
`LineEdit` uses this path, with no control-specific layout constructor. Native
handlers receive the current settled hover as an
explicit dispatch input; the host suppresses that target outside its owning view.
`CanvasSink` is the object-safe primitive drawing interface implemented by
Vello, Canvas2D, and recorders; `Canvas` adds generic convenience methods.
Native render closures outlive measurement without fixing a rendering backend
or constructing GID drawing data.
Document-aware widgets live inside Progred. During preparation they borrow
the current sources, selection, view, and path. Their handlers capture only
the props and location they need, receive `&mut Editor` at dispatch, and call
ordinary [editing helpers](../progred/src/editing.rs). There is no per-widget
dictionary of editor callbacks. A read-only line installs no editing handlers.
Puri's text editor remains independent of these document operations.
Ordinary decorations do not resolve paths or inspect selection.
A fragment is not a Canvas: it retains
whole-widget render continuations, then executes their draw calls directly after
hover settles, rather than allocating a deferred closure per drawing operation.

The native completion card uses those same outputs. Its rows draw directly
through `CanvasSink`; it never needs a document resolver or Grap interpreter.
The [container combinators](../progred/src/display/widget/container.rs) share scrolling
and out-of-flow placement over the shared `HoverPass` output. The editor's
`Placed` aliases that continuation; `Ready` aliases its returned `Fragment`.
`Layers` supplies clipping and floater attachment. Hover callbacks compose input
handlers through `HasHandler`. The editor adds view ownership separately;
running the hover pass raises floaters before querying targets. Clips do not
capture floating subtrees.

The [navigation combinator](../progred/src/display/widget/navigation.rs) similarly
contributes a projection-declared path, settled rectangle, and arrival handler.
It maps a child's hover continuation and consumes the control's arrival override
only at the nearest landmark. The native output can carry complete landmarks;
view attribution remains the editor's separate wrapper. Unplaced subtrees
contribute neither geometry nor navigation.

`widget::before` and `widget::after` contribute the same native outputs below
or above an arbitrary child. Their preparation functions capture current inputs,
then return a hover callback to run over settled placement; layout knows neither
the control nor its event policy. Click, activation, and picking are ordinary functions built
on this combinator. Hover claims, occlusion, and optional hover feedback are
ordinary decorators too. Generic Puri probes test settled hit geometry and
retention immediately inside that callback; the frame retains the winning claim,
not the probes. The editor adds the owning view. Insert and collapse handles request
feedback explicitly, not through a target-type switch in the interpreter.
Interaction wrappers use `before`, keeping child handlers in front of enclosing
handlers. `after` reverses that order when requested; borders use it to paint
above content without installing handlers. Only the chosen alternative invokes
its placement callbacks.

`libraries::layout::on_event` is the Grap adapter over that same interface.
It encodes events and installs one native handler that calls the
site-scoped interpreter directly. Ordinary native widgets do not touch
this interpreter. Layout has no Grap-event constructor or interpretation arm.

The [layout builder interface](../progred/src/display/builder.rs) belongs to
`progred::display`. `Layout` is a reusable program over that interface; its
production interpreter prepares the choice graph, while a test-only recorder
retains structure for inspection. There is no production layout enum.
Projection recursion uses ordinary preparation functions with an explicit
source scope, not path-bearing Layout variants. Native widgets and the editor
share one placement output; there is no editor-side Layout interpreter.
See [layout and widget continuations](layout-continuations.md) for the complete
chain and the distinction between preparation and placement.

Pane sizing precedes content projection. Ordinary document panes scroll over
content-sized output; explicit viewport panes pass their assigned size to a
content function and clip its output without adding padding or scrolling.
Both use the same placement, clipping, and handler contracts; viewport functions
do not add a stretch/flex policy to the baseline layout algebra.

`around` lets a consumer control when its subtree places; `before` and
`decorate` express ordinary placement/paint ordering. These belong to the
consumer's layout composition, not to a Puri widget's return type.

## Dispatch and hover

[`Handler`](../ui/puri/src/handler.rs) is one function over the input `Event`
enum. It receives mutable caller state and explicit dispatch inputs, returning
acceptance plus any unconsumed event. `over` tries the later contribution first
and passes its remainder to the earlier contribution. Scroll may leave part of
its delta; acceptance is preserved even when the next handler declines.
The typed `on_key`, `on_scroll`, and pointer helpers are ordinary combinators
over this interface. Wrappers forward the handler without unpacking channels.
Widgets test their own geometry; Puri does not infer acceptance from state changes.

Progred activation, picking, and raw pointer-down handlers use that same
front-to-back chain. The dispatch context supplies the settled hover target,
its owning view, and navigation data explicitly. Event acceptance controls
propagation; it does not tell the shell to infer a domain action or gesture.
The accepting handler performs the action and installs any continuation.

A hover callback returns its claim plus independent paint and event outputs.
Callbacks run over settled geometry from front to back; a direct claim or
occluder prevents lower hit queries without suppressing their other outputs.
Paint retains back-to-front order. Occlusion also consumes
pointer starts, while active motion/release can still reach their handlers.
A floating card carries its owning view even when it covers another pane.

Hover is derived for each pass. The frame is built without a resolved hover
input; deferred paint receives the answer after the hover continuation runs. `LazyPointer` is
an input-side dead-zone filter for small gaps. Pressed interactions retain the
anchor they need as explicit caller-owned gesture state.

## Frame lifecycle

The [application shell](../progred/src/lib.rs) adapts Winit events, owns the
model and pending frame, and supplies platform capabilities. Placement produces
an opaque hover continuation. Running it gives a settled claim, handlers,
navigation, and deferred paint; paint is not required to produce the handlers.
[`frame`](../progred/src/frame.rs) builds it; [`placed`](../progred/src/placed.rs)
composes its outputs.

A changed frame input remints a whole frame. The event-to-redraw pending frame
stages the already-built successor for presentation; it avoids building it
again at redraw. There is no event-specific list of changes considered
irrelevant to rendering and no partial invalidation system.

Pointer motion, pressed or unpressed, accumulates until a redraw or discrete
event. Each dispatch receives one `PointerUpdate`: `coalesced` contains earlier
observed states in order, excluding `current`, which is the latest state.
Only the latest packet's predictions survive; predictions are never applied as
observed input. Different contacts, buttons, modifiers, scales, or viewport sizes
start a new batch. Release and cancellation flush pending motion first.

Handlers choose which samples matter, without rebuilding between samples.
Number scrubbing integrates the full precision path; state-drag callbacks receive
the latest logical displacement and the earlier displacements, letting Fidget
orbit use only the latest. Raw Puri handlers receive the whole pointer update;
Grap motion events expose earlier sample records under `coalesced` alongside
their existing latest-position fields. Unclaimed touch motion uses the batch's
total displacement for ordinary document scrolling.

Leaving a window clears its hover position, not its active drag. Captured motion
and release retain their unbounded coordinates. Focus loss cancels through the
same pointer-cancellation handlers and clears the adapter's pressed state.

The approved cross-frame computation memo is caller-threaded text shaping.
Visible Grap canvas programs record once per frame, sharing commands and
source hits between hit-testing and painting. This within-frame sharing and
the layout DAG do not retain computation across frames. General dependency
tracking is deferred; see [deferred work](deferred.md).

## Drawing and testing

`Canvas` is a drawing interface over shapes, brushes, glyph runs, images, and
scoped clips. Production code can draw into Vello or Canvas2D; `DrawList`
records the same operations for inspection and replay. Rectangles remain
rectangles in recordings rather than becoming paths merely for transport.
Parley owns text shaping. Puri does not depend on a platform clipboard library.

Text leaves may request subscript typography: Puri shapes a smaller font and
reports ascent/descent relative to the surrounding baseline. Ordinary rows
then align it correctly without a new layout operation. The number projections
use this for muted representation labels outside the editable digits.

Puri's delimiter widget exposes its advance and minimum span for measurement,
then draws its stretched outline directly into the canvas at paint time. It
does not build an intermediate drawing-command list; clipped or skipped paint
does not construct a path. Its width is fixed by text size; only the vertical
shape stretches. Progred's `bracket` combines two ordinary
fixed-width widgets and a child in a row, with `fill_height` on the sides.
There is no `Surround` operation, maximum-width reservation, or child-dependent
remeasurement. `selectable_bracket` explicitly composes `selectable_widget`
inside the stretching wrapper, so its handlers use the expanded rectangle too.
Structural cell/list/record and expression projections opt into that behavior;
Grap's low-level bracket layout is inert unless explicitly wrapped.

`puri-widgets::panel` supplies the common fill/border painter for popup cards
and projection borders. Colors, stroke, radius, placement, paint order, and
whether the surface blocks input are supplied by the host. Panel drawing owns
no child layout, popup policy, or document state.

Color controls likewise paint directly through `CanvasSink`: their gradients,
checkerboard, swatch, and markers are not intermediate command lists. Progred
supplies the fixed metrics, scale, and point handlers. The paint-only
`widget::paint` combinator attaches one deferred painter to an extent, without
hover or document access; delimiters and Fidget image leaves use it too. Fidget
still renders during projection, then its leaf submits that image at paint time.
This changes neither Fidget's evaluation timing nor its camera behavior.

The `Drawing` description remains for explicitly stored drawing data decoded
by the layout library. Its interpreter is not used by these native widgets.

`puri-widgets::text_frame` measures empty frames using the caller's font and
supplies the outline geometry used around text. The ordinary `widget::empty`
function and `slot` combinator use it, as do pending values and their selection
outlines; there is no empty-slot layout opcode. Completion behavior and the meaning of an empty slot remain
in Progred; the widget owns only metrics and drawing.

Tests can drive pure descriptions and handlers without a window. Projection
fixtures, interaction regressions, SVG export, and profiling live in
[`projection/tests`](../progred/src/projection/tests/mod.rs). SVG tests are a
focused filter rather than the home of all interaction regressions:

```sh
./tools/sandbox-cargo test -p progred svg_bench
```

This writes sample SVG files under `target/`. The ignored IoP profiling loops
are in [frame/profile.rs](../progred/src/projection/tests/frame/profile.rs).
See [build security](build-security.md) for sandboxed commands. Agents do not
launch the application; the owner performs visual testing.
