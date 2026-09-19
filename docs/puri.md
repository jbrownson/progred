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
| [puri](../ui/puri/src/lib.rs) | Canvas and text vocabulary, placement geometry, hover claims, optional after-hover composition, typed handlers, pure widget descriptions |
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

Progred composes boxes by width, ascent, and descent. Rows align baselines,
top edges, or centers; top-aligned rows and columns choose a child's baseline.
Wrappers pad, overlay, or decorate the result.
[`measured::choices`](../ui/measured/src/choices.rs) settles
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
whose result feeds settled placements into a running `HoverPass`. Each leaf
answers hover immediately and contributes a continuation for after hover settles.
`finish` returns `HoverOutput`; binding its continuations produces paint and
handlers independently. Puri supplies generic `AfterHover<H, O>` composition,
without knowing a layout system or Progred's source identities.
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
The location-facing callbacks capture an
[editing scope](../progred/src/editing/scope.rs). At dispatch, an opaque borrowed
`Access` combines with that scope to create an `Edit` interface. Opening it
neither allocates nor copies the editor. The scope interprets locations with
an optional-path-returning `Conject`, not arbitrary replacements of every editor
operation; the normal projection uses its allocation-free identity case.
Selection and annotations remain local to the occurrence, while document reads
and mutations resolve through the same conject. Document-editing shell commands
use the scope retained by the selection too. Copy and fold are instead supplied
by the current projection, using its displayed value and fold default without
requiring a document source. A detached occurrence has no document source;
read-only behavior follows from that rather than a widget-specific write veto.
Ordinary decorations do not resolve paths or inspect selection.
A hover output is not a Canvas: it retains
whole-widget render continuations, then executes their draw calls directly after
hover settles, rather than allocating a deferred closure per drawing operation.

The native completion card uses those same outputs. Its rows draw directly
through `CanvasSink`; it never needs a document resolver or Grap interpreter.
The [container combinators](../progred/src/display/widget/container.rs) share scrolling
and out-of-flow placement over the running `HoverPass`. `Layers` supplies
clipping and floater attachment. Hover callbacks compose input handlers through
`HasHandler`. The editor adds view ownership separately. Ordinary probes run in
painting order; floating placements run afterward, outside ancestor clips.

The [navigation combinator](../progred/src/display/widget/navigation.rs) similarly
contributes a projection-declared path, settled rectangle, and arrival handler.
It scopes a child's placement output and consumes the control's arrival override
only at the nearest landmark. The native output can carry complete landmarks;
view attribution remains the editor's separate wrapper. Unplaced subtrees
contribute neither geometry nor navigation.

`widget::before` and `widget::after` contribute the same native outputs below
or above an arbitrary child. Their preparation functions capture current inputs,
then return a hover callback to run over settled placement; layout knows neither
the control nor its event policy. Click, activation, and picking are ordinary functions built
on this combinator. Hover claims, occlusion, and optional hover feedback are
ordinary decorators too. Generic Puri probes test settled hit geometry and
retention immediately inside that callback; the frame retains both the winning
claim and those probes for targeting later pointer input. The editor adds the
owning view. Insert and collapse handles request
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
and passes its remainder to the earlier contribution. Scroll carries an ordered
batch of original packets, including positions, timestamps, modifiers, and units.
`on_scroll_batch` receives that batch intact. A handler may sum it, apply
acceleration, or use the sample-wise `on_scroll` adapter. Partial consumption
forwards the ordered unconsumed packets, with adjusted deltas where necessary;
acceptance survives even when the next handler declines. Clipping divides a
batch only at crossings of its bounds, retaining in-bounds runs as batches.
Native pointer gestures use the same ordered-batch contract through
`Event::Gesture`, `on_gesture_batch`, and the sample-wise `on_gesture` adapter.
`interact::on_pinch` adds placement/clipping checks; widgets own the zoom policy.
Pinch deltas are fractional scale changes, not pixels or scroll distances.
The typed helpers are ordinary combinators over this interface.
Widgets test their own geometry; Puri does not infer acceptance from state changes.

Progred activation, picking, and raw pointer-down handlers use that same
front-to-back chain. The dispatch context supplies the settled hover target,
its owning view, and navigation data explicitly for every event, including
motion, scroll, release, and IME. View wrappers hide another view's hover without
blocking input needed by active gestures. Event acceptance controls
propagation; it does not tell the shell to infer a domain action or gesture.
The accepting handler performs the action and installs any continuation.

A hover callback returns its claim plus independent paint and event outputs.
Callbacks run over settled geometry in painting order; a later direct claim or
occluder supersedes an earlier claim, while retention cannot displace a direct
claim. Occlusion also consumes
pointer starts, while active motion/release can still reach their handlers.
A floating card carries its owning view even when it covers another pane.

Hover is derived for each pass. The frame is built without a resolved hover
input; deferred paint receives the answer after the hover continuation runs. `LazyPointer` is
an input-side dead-zone filter for small gaps. Pressed interactions retain the
anchor they need as explicit caller-owned gesture state. While a press holds
the prior hover, its owning view is retained too, both when probing installed
geometry and when building a successor. Release resumes normal probing.

## Frame lifecycle

The [application shell](../progred/src/lib.rs) adapts Winit events and supplies
platform capabilities. Each window owns an `EditorRunner`: the mutable `Editor`
beside its saved `FrameState` (dispatch, settled hover, and pending painting).
Widget handlers receive only `&mut Editor`, not their own saved dispatch. The
runner borrows those separate fields directly and owns the input batches and
frame replacement. Window setup builds the initial frame; reset uses a no-op
dispatch until the successor is built.

`HoverChanged` and `ModifiersChanged` use the ordinary Puri event chain, with
the settled hover supplied in the caller's dispatch context. Progred emits a
hover notification when the target or its owning view changes. Pointer motion
first probes the installed frame's geometry, allowing the notification to run
before building a successor. The successor still computes its own hover from
fresh geometry. An accepted
notification builds one successor; any further hover reaction waits for an
actual paint/submission before continuing. Oscillating reactions yield across
painted frames rather than recursively dispatching, panicking, or reaching an
iteration cutoff. The runner remembers the last notified identity and the paint
boundary, not a queue of handlers from discarded frames. Intervening input can
replace an unpainted successor, and the notification uses the current frame.
See [the scheduling contract](layout-continuations.md#event-and-frame-cycle).

Placement runs hover contributions over settled geometry, then the resolved
hover binds independent handlers and deferred painting. Paint is not required
to produce the handlers. [`frame`](../progred/src/frame.rs) builds and installs
the frame; [`input`](../progred/src/input.rs) interprets input into its successor.
See [layout continuations](layout-continuations.md) for the phase boundaries.

A changed frame input remints a whole frame. The event-to-redraw pending frame
stages the already-built successor for presentation; it avoids building it
again at redraw. There is no event-specific list of changes considered
irrelevant to rendering. Explicit library computations may reuse results through
the caller-owned [dependency graph](incremental.md); frame construction still runs.

Pointer motion, pressed or unpressed, accumulates until a redraw or discrete
event. Each dispatch receives one `PointerUpdate`: `coalesced` contains earlier
observed states in order, excluding `current`, which is the latest state.
Only the latest packet's predictions survive; predictions are never applied as
observed input. Different contacts, buttons, modifiers, scales, or viewport sizes
start a new batch. Release and cancellation flush pending motion first.

An active projection gesture exposes `advance` and `finish`. The shell finishes
it on release, cancellation, replacement, history restoration, or a successful
save. Its implementation owns any finalization; the frame pipeline has no
number-specific presentation channel. The number library's scrub widget keeps
precision-aware spelling in the selected line editor during the gesture and
clears that editor on finish, leaving other selection payload fields intact.
Value-changing gestures use a concrete edit run holding the target and undo
grouping flag, not a dictionary of editor callbacks.

Handlers choose which samples matter, without rebuilding between samples.
Number scrubbing integrates the full precision path; state-drag callbacks receive
the latest logical displacement and the earlier displacements, letting Fidget
orbit use only the latest. Raw Puri handlers receive the whole pointer update;
Grap motion events expose earlier sample records under `coalesced` alongside
their existing latest-position fields. Scroll also reaches the handler as one
batch before a single successor-frame build; the shell never sums or replays it.
Sample-wise controls read current caller-owned state so successive samples do
not overwrite each other. Grap scroll events expose the packet records as a list
under `content`. Unclaimed touch motion produces a scroll batch from the observed
position differences, preserving reversals and sample metadata.
Pinch/rotation samples also wait for the redraw boundary, interleaving with
pointer refreshes without forcing intermediate builds. Switching between scroll
and gesture input flushes the earlier batch; gesture end/cancel also flushes.
Grap handlers receive `gesture` events with sample records under `content`;
each names `pinch` (fractional scale) or `rotation` (clockwise radians) and `delta`.

Leaving a window clears its hover position, not its active drag. Captured motion
and release retain their unbounded coordinates. Focus loss cancels through the
same pointer-cancellation handlers and clears the adapter's pressed state.

Cross-frame reuse consists of caller-threaded text shaping and the explicit
caller-owned [computation graph](incremental.md), currently used by CAM geometry.
Visible Grap canvas programs record once per frame, sharing commands and
source hits between hit-testing and painting. This within-frame sharing and
the layout DAG do not reuse computation to construct later frames. The installed
frame retains its hit tests, including recorded drawing shapes, alongside its
handlers for subsequent input targeting. Replacement drops both. Neither Puri
nor layout owns the computation graph or decides which library results to retain.

## Drawing and testing

`Canvas` is a drawing interface over shapes, brushes, glyph runs, images, meshes, and
scoped clips. Production code can draw into Vello or Canvas2D; `DrawList`
records the same operations for inspection and replay. Rectangles remain
rectangles in recordings rather than becoming paths merely for transport.
Parley owns text shaping. Puri does not depend on a platform clipboard library.

The native canvas now groups contiguous vector operations into Vello scenes,
interleaved with independent image textures and mesh viewports in painting order. This policy
belongs to the [compositor](../ui/puri-vello/src/compositor.rs), not panes,
widgets, or layout. Vector-only frames keep a single Vello call. Mixed clipping
scopes preserve group coverage: integer rectangles use scissoring, while other
clips use an intermediate group and a Vello-rendered mask. Draw-image operations
do not enter Vello's image atlas; image brushes still do.

Each window owns its uploaded-image resources, keyed by immutable image blob
identity, dimensions, and format. Unchanged uploads are reused; absent images
are released on the next paint. Scratch textures are resized and reused, with
masks and clip-group textures retained only when used by the current frame.
These are graphics resources, not cached projections or computations. The
device's compositor is shared across windows, without sharing their resource
lifetimes. `draw_mesh` carries shared triangle geometry, an orthographic view,
and optionally depth-image replacement of a draft surface. The native compositor
draws it on the same device and consumes the resulting premultiplied texture
without a CPU round trip. Its renderer retains one mesh upload and surface
color/depth upload, not evaluated widgets or raster results. `DrawList` and the
initial `Drawing` representation retain the mesh operation; the default canvas
interpretation rasterizes it on the CPU for browser/export consumers.
See [image composition measurements](vello-compositor-experiment-2026-09-18.md)
and [direct mesh measurements](fidget-hybrid-2026-09-18.md#direct-mesh-composition).

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
hover or document access; delimiters and Fidget viewport leaves use it too.
Synchronous implicit previews render during projection, then submit an image at
paint time. Mesh views prepare shared geometry and defer rasterization to the
canvas backend. CAM computation graphs still own model/stock meshing and async
implicit work; camera and interaction policy remain in Progred.

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
