# Controls feeding a viewport

The `controls` library supplies `with controls`, a viewport composition function.
It takes `controls` (a zero-argument Grap callable), `view` (a callable), `value`,
`width`, and `height`, and returns an ordinary GID declaration. Its partial:

1. Runs `controls` with evaluation-local control capabilities.
2. Measures the emitted native widgets.
3. Calls `view` with the original `value`, full `width` and `height`, and the
   controls function's ordinary return value under `parameters`.
4. Projects the returned view and overlays the controls along its bottom edge.
   The control surface paints above the view, clips to the pane, and blocks
   pointer starts from reaching the view, including between controls.

`parameters` can be a scalar, list, record, or any other ordinary Value. A
controls function can use Grap composition and loops, request several controls,
and return their values in its own arrangement. No opaque Rust widgets pass
through Grap. Native widgets are emitted into a local collection of functions,
not a widget enum or persisted description. A halted or absent control result
discards that collection and projects the failure.

The `slider` capability requires a `key` cell identity and accepts
f64 `minimum`, `maximum`, and `initial` (defaults 0, 1, 0). Pass the identity as
data, e.g. via `quote`. The capability emits an unlabelled native slider and
returns its current f64. Calling it outside `with
controls` returns `control output required`. Bounds must be finite and increasing.

The `radio` capability uses the same `key` and takes a nonempty `options` list
of ordinary `{name: text, value: Value}` records. Values must be distinct; they
can be callables or any other ordinary data, not just indices or booleans. It
emits labeled radio buttons and returns the selected value without evaluating
it. `initial` defaults to the first option and must belong to the list. Stale or
missing stored selection falls back to that initial value. It also requires the
`with controls` scope. The CAM example offers its mesh and implicit preview
callables this way, so the choice belongs to Grap, not the CAM renderer.

Control values live in the view's existing per-location annotations, under
`control state`, keyed by the supplied identities. Two views are independent;
repeating a key at the same location intentionally shares its value. Updates
preserve camera and other annotation fields, do not change the document or its
saved flag, and do not create undo steps. Removing and restoring a control at
the same location retains its annotation, like other per-location UI state.

`puri-widgets::slider` owns range mapping and painting, without editor knowledge
or retained state. The Progred adapter owns annotation writes and the
pointer/touch gesture. It uses the existing active-gesture slot; starts respect
clipping, captured motion remains unbounded, and document replacement ends the
gesture normally. Keyboard focus/navigation and accessibility are deferred; this
slider and radio group are pointer/touch controls. `puri-widgets::radio` owns
indicator painting; Progred composes text, layout, and selection handlers.

There is no cross-frame computation cache. The controls function, view function,
and preview projection all run again on each projected frame; a preview may use
the general dependency-tracked computation system for its expensive work.
Controls do not reduce the view's assigned size. Their bottom alignment comes
from their actual measured height and the settled view bounds, not a matching
constant in the example.
