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

The package boundaries are:

| Package | Responsibility |
| --- | --- |
| [puri](../ui/puri/src/lib.rs) | Canvas and text vocabulary, placement geometry, typed handlers, pure widget descriptions |
| [puri-widgets](../ui/puri-widgets/src/lib.rs) | Reusable composed widgets, including completion rows |
| [measured](../ui/measured/src/lib.rs) | Measurement and box composition with opaque placement outputs |
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
result. [`projection/choices`](../progred/src/projection/choices.rs) settles
ordered alternatives over already measured leaves. The first preferred form
whose natural width fits wins; otherwise the last form accommodates the
available width. Selection does not reshape text or rerun projections.
Shared layout nodes belong to this one frame and are consumed by the selected
form.

`around` lets a consumer control when its subtree places; `before` and
`decorate` express ordinary placement/paint ordering. These belong to the
consumer's layout composition, not to a Puri widget's return type.

## Dispatch and hover

[`Handler`](../ui/puri/src/handler.rs) composes one function per typed event
channel. Later registrations are tried first; a declined event continues to
the next handler. Widgets test their own geometry and receive mutable caller
state only at dispatch. Scroll handlers can consume part of a delta and pass
the remainder along.

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
