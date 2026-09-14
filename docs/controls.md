# Controls feeding a viewport

The `controls` library supplies `with controls`, a viewport composition function.
It takes `controls` (a zero-argument Grap callable), `view` (a callable), `value`,
`width`, and `height`, and returns an ordinary GID declaration. Its partial:

1. Runs `controls` with evaluation-local control capabilities.
2. Measures the emitted native widgets.
3. Calls `view` with the original `value`, `width`, remaining `height`, and the
   controls function's ordinary return value under `parameters`.
4. Projects the returned view and places the controls below it.

`parameters` can be a scalar, list, record, or any other ordinary Value. A
controls function can use Grap composition and loops, request several controls,
and return their values in its own arrangement. No opaque Rust widgets pass
through Grap. Native widgets are emitted into a local collection of functions,
not a widget enum or persisted description. A halted or absent control result
discards that collection and projects the failure.

The first capability is `slider`: it requires a `key` cell identity and accepts
f64 `minimum`, `maximum`, and `initial` (defaults 0, 1, 0). Pass the identity as
data, e.g. via `quote`; its current graph name supplies the label. The capability
emits a native slider and returns its current f64. Calling it outside `with
controls` returns `control output required`. Bounds must be finite and increasing.

Slider values live in the view's existing per-location annotations, under
`control state`, keyed by the supplied identities. Two views are independent;
repeating a key at the same location intentionally shares its value. Updates
preserve camera and other annotation fields, do not change the document or its
saved flag, and do not create undo steps. Removing and restoring a control at
the same location retains its annotation, like other per-location UI state.

`puri-widgets::slider` owns range mapping and painting, without editor knowledge
or retained state. The Progred adapter owns its label, annotation writes, and
pointer/touch gesture. It uses the existing active-gesture slot; starts respect
clipping, captured motion remains unbounded, and document replacement ends the
gesture normally. Keyboard focus/navigation and accessibility are deferred; this
first slider is a pointer/touch control.

There is no cross-frame computation cache. The controls function, view function,
and preview all run again on each projected frame. The assigned height is reduced
by the actual measured controls, not by a matching constant in the example.
