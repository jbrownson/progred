# Puri and the editor frame

This describes the current code and its boundaries;
it does not attribute the earlier framework comparisons or future plans to the
owner. Those notes are retained in [history](history/puri-notes.md).

## Ownership

Puri provides ephemeral widget descriptions, drawing, interaction helpers,
and caller-owned state types. A description receives state and presentation
inputs, then uses settled geometry to draw and register handlers. Puri holds
no widget or application state between frames, retains no application
hierarchy, mints no identity, and owns no global clipboard, window, clock, or
focus service. Platform services arrive only as caller-supplied capabilities in
the dispatch context. Puri grows only as Progred needs it.

Masonry is a quarry, not a foundation: Puri inherits none of its widget tree,
pods, or context protocol. Code taken from it is vendored with attribution and
purified in place; trivial widgets are rewritten instead. None is vendored
today: line editing borrows Parley's `PlainEditor` behavior, constructed
transiently for each pass and dispatch.

The caller owns focus, any focus order, and durable interaction state; a
description receives focus as an input. Durable helper state holds only
caller-owned content and cross-frame interaction data. `LineEditState` contains
text and cross-frame editing state; `LineEditDescription` supplies font, paint,
affixes, focus, placeholder, and chrome for this description, and its transient
handlers capture those presentation inputs. `EditCtx` supplies
mutable state, Parley contexts, a clipboard capability, and the host's command
modifier at dispatch. A WebAssembly host chooses that modifier from the browser's
platform rather than the compilation target. A focused editor
reports its caret as the frame's input area. The native shell allows platform
IME only while a frame has one and places composition candidates there; the
browser host does not enable IME yet.

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
| [puri-web](../ui/puri-web/src/lib.rs) | Browser Canvas2D fallback (WebGPU uses puri-vello) |
| [Progred placement](../progred/src/display/widget/frame.rs) | Editor hover, navigation, popup policy, deferred paint, and dispatch inputs |

Reusable widgets do not interpret document values or choose domain completion
vocabulary. Progred and its projection libraries supply those choices. The Puri
runtime carries no widget catalog; `puri-widgets` depends only on Puri and stays
a pure consumer of it.

## Measurement and placement

Puri owns `Placement { rect, available_rect, clip_rect }`, not a layout algebra.
Measurement composition, layout nodes, containers, and traversal belong to the
consumer. Puri text and editor descriptions expose metrics and accept a settled
placement; interaction helpers register against one.
`rect` is the widget's rectangle. `available_rect` is an optional expansion
offered by its container: rows offer their vertical span, columns their width,
and overlays both. Ordinary widgets ignore it. A `fill_height` combinator adopts
the available vertical span before invoking the child's placement continuation.
This does not change intrinsic measurement or trigger another choice search.
Available space is neither a clip nor a hit target.
`clip_rect` is the effective enclosing axis-aligned clip,
not already intersected with the widget. Ordinary children inherit the clip;
clipping containers intersect their bounds into it. Clipping travels only in
each placement, never as mutable context. Hover and gesture starts
must lie inside both rectangles. Motion and release for an active gesture can
continue outside them.

Canvas clips are separate: they constrain ink and can use arbitrary shapes.
An axis-aligned layout clip neither describes nor replaces an arbitrary canvas
clip. Clipping does not in itself remove navigation or active handlers.

Layout places boxes with opaque continuations. Projection, navigation,
selection, hover policy, and event interpretation are not box primitives.
Progred composes boxes by width, ascent, and descent. Rows align baselines,
top edges, or centers; top-aligned rows and columns choose a child's baseline.
Wrappers pad, overlay, or decorate the result. With ordered alternatives, this
is Progred's whole layout algebra; there is no general layout engine. Anchors
and clearance derive from the same metrics, outlines, and stroke widths used to
draw. Padding and gaps are explicit styling inputs, never offsets tuned to
compensate for borders or other geometry owned elsewhere.
[`measured::choices`](../ui/measured/src/choices.rs) settles
ordered alternatives over already measured leaves. The first preferred form
whose natural width fits wins; otherwise the last form accommodates the
available width, unless an earlier form's natural width is no wider than that
accommodation, in which case the earlier form wins at its natural width. A form
chosen at its natural width places every nested alternative in its first form.
Selection does not reshape text or rerun projections.
Shared layout nodes belong to this one frame and are consumed by the selected
form.

The choice engine has no GID, editor, or Puri dependency. `ChoiceBuild` owns
per-frame sharing and choice bookkeeping; `resolve_choices` returns a
`Measured<Out>`: an extent and placement callback. The selected choice graph
places directly, without building a second measured container tree. Plain
measured combinators compose callbacks using the same geometry helpers, not a
`Kind` enum. `ObserveLayout` exposes the chosen row/column/layers and direct-child
boundaries during that placement, for consumers that need structural traversal.
It does not expose discarded alternatives or require another tree. Wrappers do
not interpret their output. Out-of-flow
content uses `attach`: only the base contributes to surrounding width, and
the consumer supplies how the two settled subtrees place. Popover styling,
position, occlusion, and raising remain Progred policy.
The display builder's `floating` operation supplies only two boxes and a
positioning function. The ordinary [popover widget](../progred/src/display/widget/popover.rs)
composes its padding, panel ink, and input blocking explicitly; the layout
interpreter does not add these to a floating box.

The [layout builder interface](../progred/src/display/builder.rs) belongs to
`progred::display` and accepts box composition and opaque leaf and preparation
functions. Rows, columns, overlays, padding, floating, sharing, and alternatives
own geometry. `before` and `after` compose placement work without interpreting
it. `Layout` is a reusable program over that interface, not an enum
description.

[`display::measure`](../progred/src/display/measure.rs) is the production
interpreter: it prepares the choice graph. Its `Node` results are temporary
interpreter-local slots, consumed by parent operations; they are not persistent
identities or paths. Shared children use an explicit sharing key and prepare
only once per frame. The selected graph invokes its leaves directly, and opaque
wrappers receive a child's extent and placement callback once choices have
settled. [`recording`](../progred/src/display/recording.rs) is a test-only
implementation of the same calls that retains structure for inspection without
executing opaque widget programs; production has no layout enum. Layout
programs are reusable closures, so their captures and the choice graph still
allocate; this is not an allocation-free or entirely streaming layout engine.

Pane sizing precedes content projection. Ordinary document panes scroll over
content-sized output; explicit viewport panes pass their assigned size to a
content function and clip its output without adding padding or scrolling.
Both use the same placement, clipping, and handler contracts; viewport functions
do not add a stretch/flex policy to the baseline layout algebra.

## Widgets and hover continuations

Native widgets use `progred::display::widget::Widget`: a measurement function
whose result feeds settled placements into a running
[`HoverPass`](../progred/src/display/widget/frame.rs). A `Program` can instead
prepare a subtree with choices, using the same per-frame `ChoiceBuild`. Neither
is a catalogue of control variants; `LineEdit` uses this path with no
control-specific layout constructor. Puri's plain text and drawing leaves are
measured by [`widget::drawing`](../progred/src/display/widget/drawing.rs) and
contain no document or interaction information.

Each leaf answers hover immediately and contributes a continuation for after
hover settles; there is no catalogue of ordinary hover callbacks. `HoverContext`
is the transient interface a widget uses to answer probes, declare navigation,
and supply `after_hover` continuations. `render` is a combinator over that boundary: native handlers that don't need the
winner are lifted into it at the end of the widget's contribution, and handlers
that do are constructed inside `after_hover`. `finish` runs floating placements
and returns `HoverOutput`. Its consuming `bind` uses `Attribution` to assemble
`Effects` and returns a distinct `FrameOutput`: canvas-only renders, one
function-over-`Event` handler chain, and settled navigation, view, and hover
geometry. A hover output can't be bound twice or masquerade as a completed
frame. Nor is it a Canvas: it retains whole-widget render continuations and
executes their draw calls directly after hover settles, rather than allocating
a deferred closure per drawing operation. Native render closures outlive
measurement without fixing a rendering backend or constructing GID drawing data.

[`puri::frame::AfterHover<H, O>`](../ui/puri/src/frame.rs) owns this reusable
phase composition. Its resolved input and output are generic, with no
dependency on Progred, its identities, or `measured`; it is optional plumbing
for widgets, not a required Puri protocol. Puri also owns probes and claim
precedence. Progred owns source attribution, view identity, navigation, and
popup policy. Native handlers receive the current settled hover as an explicit
dispatch input; the host suppresses that target outside its owning view.

Hover and continuation collection run in painting order, with no reversed paint
or navigation segments. Scoped wrappers map a child's output, consuming
navigation overrides without affecting siblings or ancestors. Floating
placements escape enclosing clip and navigation scopes but retain their owning
view.

Progred's `attribute` function reads the hovered and selected occurrences'
secondary identities off their landmarks, which recorded them during layout,
and derives the hovered source trace once. `Attribution` shares those results
within the frame, so each painted occurrence only compares. It is constructed
for each frame, not retained as a memo.

Document-aware widgets live inside Progred. During preparation they borrow
the current sources, selection, view, and path, requesting document-site state
only when needed; inert decorators construct no editing state or capabilities.
Their handlers capture only
the props and location they need, receive `&mut Editor` at dispatch, and call
ordinary [editing helpers](../progred/src/editing.rs). There is no per-widget
dictionary of editor callbacks. A read-only line installs no editing handlers.
Puri's text editor remains independent of these document operations.
The location-facing callbacks capture an
[editing scope](../progred/src/editing/scope.rs). At dispatch, the scope opens
an `Edit` interface over the session the dispatch lends. Opening it
neither allocates nor copies the editor. The scope interprets locations
through conjects, a jump's location or a computed value's none, not arbitrary
replacements of every editor operation; the normal projection uses its
allocation-free identity case.
Selection and annotations remain local to the occurrence, while document reads
and mutations resolve through the same conject. Document-editing shell commands
use the scope retained by the selection too. Copy and fold are instead supplied
by the current projection, using its displayed value and fold default without
requiring a document source. A detached occurrence has no document source;
read-only behavior follows from that rather than a widget-specific write veto.
Ordinary decorations do not resolve paths or inspect selection.

The native completion card uses those same outputs. Its rows draw directly
through `CanvasSink`; it never needs a document resolver or Grap interpreter.
The [container combinators](../progred/src/display/widget/container.rs) share scrolling
and out-of-flow placement over the running `HoverPass`. `Layers` supplies
clipping and floater attachment. Hover callbacks compose input handlers through
`HasHandler`. The editor adds view ownership separately. Ordinary probes run
immediately in painting order; only floating placements are queued, running
afterward, outside ancestor clips.

The [navigation combinators](../progred/src/display/widget/navigation.rs) supply
selectable leaf and whole-value stops. Progred consumes the box engine's
`ObserveLayout` traversal, folding each child into line summaries as it finishes:
columns concatenate, while rows align the declared logical baselines. After
placement only the selected occurrence's four destinations remain, in one
ordinary `Event::Navigate` handler; no graph or logical layout survives in the
installed frame. Landmarks separately retain rectangles for selection and
reveal, and a control's arrival override is consumed by its nearest landmark.
Discarded alternatives contribute no navigation, hover, handlers, or painting.
Puri does not know this policy; it still receives ordinary events and handlers.
[Navigation](navigation.md) covers how whole values and multiline blocks join
lines, dispatch, the arrow policy, and arrival.

The [projection helpers](../progred/src/display/projection.rs) only declare
whole-value and leaf stops and compose common inline-or-indented head/body
layouts. They use the ordinary layout functions, with no navigation-specific
row/column wrappers. `group(content)` uses the generic `widget::around`
placement boundary, not a layout opcode.

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

`around` lets a consumer control whether or when its subtree places; `before` and
`decorate` express ordinary placement/paint ordering, including ordinary
leading work, without special cases. These belong to the
consumer's layout composition, not to a Puri widget's return type.

## Projection recursion and Grap layout

`descend`, `at`, and `jump` build ordinary preparation functions using an
explicit [projection scope](../progred/src/display/widget/project.rs). The app
supplies source lookup, cycle detection, conject, and the selected current and
descendant partials. There are no path-bearing layout operations, and the editor
doesn't interpret layouts; native widgets and the editor share one placement
output.

Preparation interleaves projection and measurement. These functions run before
choice resolution and placement, not eagerly while a partial first constructs
its description. That retains sharing: a descendant referenced by several
alternatives prepares once in that frame. The next frame starts a fresh builder.

`current` and `default` remain separate explicit projection inputs. Missing
locations pass `None` to the chosen partial and fall back to the ordinary
pending widget; computed roots remain read-only. The GID layout library's
path-bearing forms decode through the common path library into these functions,
not a second path opcode interpreter.

Grap can return stored GID layout forms for decoding, or produce an explicit
`layout program`. The latter holds an ordinary Grap closure; its partial applies
that closure with borrowed FFIs closed over a Rust output buffer. Box and widget
calls emit native layouts into that buffer, never native objects or handles into
Grap. Grouping calls evaluate a raw body while collecting its children, then
emit their parent. The buffer is evaluation-local, and a halt or invalid builder
call discards it. See [the scoped interface](../progred/src/libraries/layout/scope.rs).

The effect boundary is explicit so existing value-returning projection
combinators keep their semantics. For example, `border` can wrap a returned
layout program just as it wraps another projected value. It does not inspect
or intercept the builder's effects. Native widgets do not take either GID path,
and neither path encodes every drawing operation as GID before painting.
`libraries::layout::on_event` is the Grap adapter for handlers: it installs one
native handler that encodes each event as the core's `site::event` record and
calls the site-scoped interpreter directly. Layout has no Grap-event
constructor or interpretation arm.

Line editing, delimiters, pointer actions, hover feedback, scrubbing, state
scrolling, borders, and popovers are native widget functions and combinators.
Completion and drawing-program functions call app adapters during preparation;
the adapters own document-specific offers, evaluation, and source attribution,
and return the same measured widget output. They are not operations of the box
interpreter. Completion providers remain explicit lazy inputs and run only for
the active picker.

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

Scroll containers also emit read-only capture probes with their settled placement,
scale, offset, and limits. The web embed's `wheel=auto` listener consults these
before allowing winit to receive the event: any consumable scroll captures the
whole event, otherwise the browser keeps it. Probes reuse the offset/clamping
calculation and ancestor clips; normal handlers still choose the frontmost
consumer and propagate internal remainders. Probes are snapshots of the installed
frame, so a batch can briefly continue capturing after reaching an edge, until
its successor is installed. They do not dispatch handlers, mutate state, or force
a build. Native dispatch ignores this output. Custom wheel actions such as camera
zoom remain available in the full editor's unconditional `wheel=editor` mode.
The typed helpers are ordinary combinators over this interface.
Widgets test their own geometry; Puri does not infer acceptance from state changes.

Progred activation, picking, and raw pointer-down handlers use that same
front-to-back chain, not separate dispatch phases. The dispatch context
supplies the settled hover target, its owning view, reveal geometry, and the
pointer, modifiers and button state as the event finds them, explicitly for
every event, including motion, scroll, release, and IME. View
wrappers hide another view's hover without blocking input needed by active
gestures. Event acceptance controls
propagation; it does not tell the shell to infer a domain action or gesture.
The accepting handler performs the action and installs any continuation.
Gesture startup is an explicit update to caller-owned state: selection and drag
startup compose in the same accepted interaction, rather than a gesture being
inferred from an unrelated handler returning true.

A hover callback returns its claim plus independent paint and event outputs.
A `puri::hover::Claim` either names a target, directly or by retention, or
occludes: the claim analog of an opaque fill.
Callbacks run over settled geometry in painting order; a later direct claim or
occluder supersedes an earlier claim, while retention cannot displace a direct
claim. Occlusion also consumes
pointer starts, while active motion/release can still reach their handlers.
A floating card carries its owning view even when it covers another pane.

Hover is derived for each pass. The frame is built without a resolved hover
input; deferred paint receives the answer after the hover continuation runs. Pressed interactions retain the
anchor they need as explicit caller-owned gesture state. While a press holds
the prior hover, its owning view is retained too, both when probing installed
geometry and when building a successor. Release resumes normal probing.

A callback can also name the pointer's cursor for its own target, as a divider
does for its resize arrows. The frame keeps those requests beside its probes,
and the shell shows the hovered target's, or a running gesture's own while one
runs. Callbacks also learn whether the command modifier is held, so a widget
can show what holding it allows.

## Frame lifecycle

The [application shell](../progred/src/app.rs) adapts Winit events, supplies
platform capabilities, and schedules painting; it contains no keyboard chain or
pointer selection rules. Each window owns an `EditorRunner`: the mutable
`Editor` beside its `FrameState` (dispatch, settled hover, and optional pending
painting), plus queued continuous input. Widget handlers receive only
`&mut Editor`; the saved handler is never part of the state passed back to
itself.

[`input.rs`](../progred/src/input.rs) interprets keyboard, pointer, and IME
events. Its `update_frame` calls run the installed frame's handler, then build
its successor. `update_frame` takes an ordinary event-interpreting function, not
another event encoding or an action queue. It borrows the editor mutably and the
previous dispatch and hover immutably, then installs the successor after
dispatch returns; there is no temporary no-op substitution, handler clone, or
restore step. Dispatch is always present: its default, an empty handler and
navigation data, is only the initial and reset frame. Window setup builds the
initial frame once the window size is known, so event handling has no
frame-initialization branch. Unhandled events with unchanged frame inputs retain
the installed frame.

Document replacement has the same separation: the editor finalizes gestures and
replaces document-owned state; the runner clears old frame outputs, queued
input, and hover-notification state, then builds the successor immediately when
a window is active. No replacement flag or snapshot comparison is needed to
discover that the frame was invalidated.

### Building a frame

`refresh_frame` in [`frame.rs`](../progred/src/frame.rs) builds the successor:

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
   winning claim, the probes that produced it, navigation declarations, and
   `AfterHover` continuations. Widget placement callbacks are one-shot;
   only their settled hit tests are retained.
5. The app constructs `Attribution` and binds the continuations. This produces
   rendering and handlers independently. Each render is a canvas-only
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

`compute_hover` owns the whole hover stage: probing, preserving the prior target
during a press, attributing the winner, and binding the continuations. It returns
a completed `Frame`; it never updates the runner's stored hover. Paint is not
required to produce the handlers.

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

### Selection and reveal

Selection and scrolling are separate operations. Navigation arrival explicitly
selects and reveals; deletion, pending insertion/cancellation, and Undo/Redo
explicitly reveal their resulting selection. Native and drawn menus use the same
commands. Navigation's [`Geometry`](../progred/src/display/widget/navigation/geometry.rs)
borrows the installed frame's landmarks, view regions, and scale; a reveal immediately adjusts the appropriate view's offset.
Ordinary pointer selection, completion, and low-level selection/payload setters
do not implicitly scroll. Widgets that navigate elsewhere can compose selection
with `reveal_path` or `reveal_selection`, as canvas picking does through `arrive`.
`update_frame` neither compares selections nor processes reveal requests.
A missing path leaves the offset unchanged, with no deferred retry.
Substantially reflowed destinations are deliberately best-effort. There is no
reveal flag, remembered target, corrective build, or new layout/hover boundary.

### Hover notifications

`HoverChanged` and `ModifiersChanged` use the ordinary Puri event chain, with
the settled hover supplied in the dispatch context. The installed dispatch owns
`HoverGeometry`: the frame's probes in painting order, including their clips,
retention policy, exact shape tests, and owning views. It uses the same probes
and precedence as placement, not navigation rectangles or a second approximate
hover algorithm. A motion batch probes this geometry at its latest position and
dispatches any `HoverChanged` before its ordinary motion handler and successor
build. This lets a hover reaction, such as revealing drawing source, contribute
to that one successor instead of first building a frame just to discover the
hover. Probing does not rerun projection or layout. Drawing probes keep the
installed frame's recorded shapes alive after painting; they do not evaluate the
next frame's drawing early or reuse the recording to build that next frame.

The successor still computes hover over fresh geometry. After installing a
different hover target or owning view, the runner dispatches
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
never a saved callback into the replaced document.

The drawing widget handles hover changes by explicitly revealing the winning
source in the navigation geometry. Its ordinary `ModifiersChanged` handler
does the same when Cmd is pressed over a stationary pointer. Both require the
link modifier, the owning view, and a pointer inside the drawing's clipped
placement, as the dispatch context reports them. The frame runner contains no canvas-specific reveal operation.

### Input batching and gestures

Pointer motion, pressed or unpressed, accumulates until a redraw or discrete
event. Each dispatch receives one `PointerUpdate`: `coalesced` contains earlier
observed states in order, excluding `current`, which is the latest state.
Only the latest packet's predictions survive; predictions are never applied as
observed input. Different contacts, buttons, modifiers, scales, or viewport sizes
start a new batch. Release and cancellation flush pending motion first.

The editor holds at most one active projection gesture, in a caller-owned
slot. The accepting handler installs it, finishing any predecessor, and it owns
motion until it finishes. Domain updates and undo grouping live in the gesture,
outside platform dispatch.
An active projection gesture exposes `advance` and `finish`. The shell finishes
it on release, cancellation, replacement, history restoration, or a successful
save. Its implementation owns any finalization; the frame pipeline has no
number-specific presentation channel. The number library's scrub widget keeps
precision-aware spelling in the selected line editor during the gesture and
clears that editor on finish, leaving other selection payload fields intact.
Value-changing gestures use a concrete edit run holding the target and undo
grouping flag, not a dictionary of editor callbacks.

Handlers choose which samples matter, without rebuilding between samples.
Batching is not a per-widget opt-in, and the shell never discards earlier
samples. Number scrubbing integrates the full precision path; state-drag
callbacks receive the latest logical displacement and the earlier
displacements, letting Fidget orbit use only the latest. Raw Puri handlers
receive the whole pointer update;
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

### Timers

Puri's timer capability creates a one-shot handle and a weak completion handle
for the shell. Dropping the owning handle cancels it; delivery clears its
deadline and cannot affect a replacement handle. Progred retains only the weak
completions, wakes at the earliest deadline through the event loop, delivers due
timers, and requests a frame. Early platform wakeups leave future deadlines
pending. Timer queues have document lifetime and retain no document callbacks.

`puri-widgets` supplies a caller-owned debounce over that capability: triggering
replaces its timer, and readiness means either delivery or elapsed monotonic
time. CAM camera handlers use it only for accepted zoom changes, with a 150 ms
delay. Each preview derives readiness at frame build time, so a missing delivery
recovers on the next frame. Only implicit pixel-job submission waits; input,
immediate mesh drawing, and other previews continue independently. The shell
knows nothing about scrolling, debounce durations, or render admission.

### Focus

Leaving a window clears its hover position, not its active drag. Captured motion
and release retain their unbounded coordinates. Focus loss cancels through the
same pointer-cancellation handlers and clears the adapter's pressed state.
Window focus is a separate frame input from document selection. Losing focus
clears selection and its transient editing state, including completion queries
and uncommitted composition. Document edits already made remain; refocusing does
not restore the old selection. Primary and related selection highlights disappear.
Keyboard and IME input require focus. In browser embeds, canvas focus and the
iframe document's focus must both be present; standard window focus/blur events
cover leaving the iframe while its canvas remains the active element. Loading an
embed does not focus it automatically.

### Reuse across frames

A changed frame input remints a whole frame. The event-to-redraw pending frame
stages the already-built successor for presentation; it avoids building it
again at redraw. There is no event-specific list of changes considered
irrelevant to rendering, and no partial invalidation. Explicit library
computations may reuse results through the caller-owned
[dependency graph](incremental.md); frame construction still runs. If whole
frames become too slow, the remedy is one general dependency-tracked
invalidation system, observing missing definitions and ordered definition sets
too, not event-specific special cases.

Cross-frame reuse consists of caller-threaded text shaping and the explicit
caller-owned [computation graph](incremental.md), currently used by CAM geometry.
Visible Grap canvas programs record once per frame, sharing commands and
source hits between hit-testing and painting. This within-frame sharing and
the layout DAG do not reuse computation to construct later frames. The installed
frame retains its hit tests, including recorded drawing shapes, alongside its
handlers for subsequent input targeting. Replacement drops both. Neither Puri
nor layout owns the computation graph or decides which library results to retain,
and neither keeps a hidden cache.

## Drawing

[`CanvasSink`](../ui/puri/src/draw.rs) is the object-safe drawing interface over
shapes, brushes, glyph runs, images, meshes, and scoped clips, implemented by
Vello, Canvas2D, and the test recorder; `Canvas` supplies generic convenience
methods above it. Whole-widget render closures call that interface directly;
there is no backend-specific fragment conversion or new GID drawing list.
`DrawList` records the same operations for inspection and replay. Rectangles remain
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
See [image composition measurements](history/vello-compositor-experiment-2026-09-18.md)
and [direct mesh measurements](history/fidget-hybrid-2026-09-18.md#direct-mesh-composition).

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

Tests drive pure descriptions and handlers without a window; see
[testing](testing.md).
