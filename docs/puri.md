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

Puri owns `Placement { rect, clip_rect }`, not a layout algebra. `rect` is the
full widget rectangle. `clip_rect` is the effective enclosing axis-aligned clip,
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
`Measured<Out>`. Its leaves own opaque placement continuations, and wrappers
compose those continuations without interpreting their output. Out-of-flow
content uses `attach`: only the base contributes to surrounding width, and
the consumer supplies how the two settled subtrees place. Popover styling,
position, occlusion, and raising remain Progred policy.

Native widgets use `progred_display::widget::Widget`: a measurement function
whose result places a `Fragment` of deferred ink, handlers, hover claims, and
navigation declarations. `LineEdit` uses this path, with no control-specific
layout constructor. Native handlers receive the current settled hover as an
explicit dispatch input; the host suppresses that target outside its owning view.
`CanvasSink` is an object-safe bridge to the existing
canvas interpreter, allowing native render closures to outlive measurement
without fixing a rendering backend or constructing GID drawing data.
The editor supplies the current text state and edit/selection capabilities;
the generic layout adapter does not parse or render line-editor props.

`widget::before` contributes the same native outputs before an arbitrary
child places. Its preparation function captures current inputs, then returns
an opaque placement callback; `Layout::Before` knows neither the control nor
its event policy. Click, activation, and picking are ordinary functions built
on this combinator. Child handlers remain in front of their enclosing handlers,
and only the chosen alternative invokes its placement callbacks.

The upper `progred_display::Layout` still mixes boxes with other deferred
editor requests. Separating that remaining layer is an
[in-progress migration](layout-continuations.md), not a completed boundary.

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

A hover probe returns a target, occlusion, or no claim. Probes are asked over
settled geometry in paint order from front to back. Occlusion also consumes
pointer starts, while active motion/release can still reach their handlers.
A floating card carries its owning view even when it covers another pane.

Hover is derived for each pass. The frame is built without a resolved hover
input; deferred paint receives the answer after placement. `LazyPointer` is
an input-side dead-zone filter for small gaps. Pressed interactions retain the
anchor they need as explicit caller-owned gesture state.

## Frame lifecycle

The [application shell](../progred/src/lib.rs) adapts Winit events, owns the
model and pending frame, and supplies platform capabilities. A frame contains
settled placement, hover probes, handlers, navigation, and deferred paint.
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

Puri's delimiter widget accepts a vertical span and text size and returns the
existing `Drawing` description with its metrics. It owns minimum glyph height,
baseline trimming, side bearings, and width growth. Progred reserves its maximum
advance while choosing layouts. `Surround` receives opaque side widgets with
width bounds, measures them against the chosen child's extent, and places the
three boxes on one baseline. It knows neither delimiters nor editor actions.
The ordinary `widget::delimiter` functions supply the ink and gap padding.
`bracket` is inert; `selectable_bracket` explicitly composes `selectable_side`,
which uses the same `selectable` measured-widget decorator available to other
controls. Structural cell/list/record and expression projections opt into that
behavior; Grap's low-level bracket layout is inert unless explicitly wrapped.

`puri-widgets::panel` supplies the common fill/border painter for popup cards
and projection borders. Colors, stroke, radius, placement, paint order, and
whether the surface blocks input are supplied by the host. Panel drawing owns
no child layout, popup policy, or document state.

`puri-widgets::text_frame` measures empty frames using the caller's font and
supplies the outline geometry used around text. Progred lowers the inert
`Layout::EmptySlot` request through it, and uses it for pending values and their
selection outlines. Completion behavior and the meaning of an empty slot remain
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
