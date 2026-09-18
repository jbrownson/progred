# Controls feeding a viewport

The `controls` library supplies `with controls`, a viewport composition function.
It takes `controls` (a zero-argument Grap callable), `view` (a callable), `value`,
`width`, and `height`, and returns an ordinary GID declaration. Its partial:

1. Runs `controls` with evaluation-local control capabilities.
2. Measures the emitted native widgets.
3. Calls `view` with the original `value`, full `width` and `height`, and the
   controls function's ordinary return value under `parameters`.
4. Projects the returned view and overlays the controls along its bottom edge.
   Only the controls themselves paint above the view; no background strip hides
   the image beneath them. The control surface clips to the pane and blocks
   pointer starts from reaching the view, including between controls.

The constructor keeps its evaluated arguments in Grap's runtime representation
until the evaluation's GID result boundary. The declaration is still data, not
an emitted widget: its partial performs the control/view work described above.
Returning that data preserves sharing between captured environments and `value`;
it does not expand a separate copy of a program tree for every captured binding.

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
`with controls` scope. Radio groups can offer preview callables this way; the
current CAM example uses them for Model / Stock, independently of automatic
mesh-to-implicit refinement, playback, and the nested-list range selector below.
The choice is ordinary Grap data passed from controls to the example's view
function. It defaults to Model and does not reset the cursor, ranges, or camera.

`tree range` takes a `key` and `items`, an ordinary nested list. Lists are groups;
every non-list value is an uninterpreted leaf. It emits equal-width, unlabelled
notched range sliders, finest first and coarsest last, and returns the selected
half-open leaf-index range as `[start, end]` (f64 integers). It does not resolve
cells or evaluate leaves. The caller supplies an already-expanded tree, and can
use the result for any domain, not just toolpaths.

Separators narrow to at most a quarter of each notch's width and disappear below
two physical pixels per notch. Dense rows retain the same selection color,
endpoint handles, and per-item interaction; only the separators change.
Tree controls show closely packed selection bands, with playback at the top and
no enclosing frame or additional group padding. Each range row has a 20-point
hit height; its 16-point band and small endpoint marks stay inside that row.
Rows still have independent coordinates and gestures, not a shared horizontal
timeline or cross-row dragging.
In a tree cursor, an amber underline marks the section containing playback in
each visible row. It is derived from the current tree and cursor, independently
of range-selection intent. At the selected range's end it marks the last included
leaf and its containing groups; very dense notches get a minimum-width marker.
The continuous slider shows boundary ticks, taller for coarser groups. Their
positions follow the actual leaf-unit playback scale, not the equal-width range
notches. A complete tick level is omitted when any adjacent visible boundaries
would be less than eight logical points apart; individual marks are never thinned.
Levels return as the selected range narrows or the viewport widens. Ticks are
24 points tall and 3 points thick at the coarsest level. Each finer level is
two-thirds as tall and thick, down to 6 points tall and 1 point thick. Coarser
levels paint last so shared boundaries retain their emphasis. These are only
visual guides: dragging remains continuous, without snapping or extra hit targets.

Each row selects contiguous children of the previous row's selected groups.
Rows align by remaining subtree depth: shallower groups remain whole while
deeper groups expand, so leaves meet on the finest row rather than appearing
early among coarser groups. Empty lists contribute no leaves. All is the initial
selection. Click a notch for one item,
drag across notches for a range, or drag either edge handle to resize it.
Initialization and double-click select explicit `All`, including future items.
A manual drag, even across every visible notch, instead records inclusive
endpoint occurrence paths. Insertions between those endpoints are included;
insertions outside them are not. Deleted endpoints remain ordered bounds, so
surviving items between them stay selected. Equal values at different list
positions remain independent occurrences.

Adjusting a row sets every finer row to `All`, including temporarily hidden rows;
coarser selections stay unchanged. In a tree cursor, clicking the amber-marked
current item is an exception: the clicked row narrows to that item while finer
selections are preserved. Dragging out into a wider range resets them; returning
to the current item during that drag does not restore the old filters. Handle
drags and double-click-to-All still reset finer selections, even when the adjusted
range happens to stay the same.
Returning to an earlier group therefore does not resurrect its old finer filter.
Moving playback does not reset ranges. Rebuilding after document edits preserves
stored intent; an out-of-range intent temporarily displays all available items.
Rows are indexed by remaining depth (finest is zero), not by their current
display offset.

Stored range state is a finest-first list of `all` cells or pairs of encoded
list-position paths, under the supplied key. It uses the existing path encoding,
not a new identity scheme. This depends on the producer preserving list positions:
Grap operations that construct new lists, including quote reconstruction and
generated iteration lists, can assign new positions on recomputation. Stable
identity across those replacements is not inferred from values or closures.

`tree cursor` combines the same range stack with a continuous slider above it.
It takes `key`, `items`, and an optional finite `initial` position (default 0),
and returns `{range: [start, end], position: f64}`. Position is in leaf units:
the integer part identifies a leaf and its fractional part the position within
that leaf. The selected end is included. Changing the selected range retains
the position if it is still inside, otherwise moves it to the new start. Its
single state record stores both range intents and a cursor pair (leaf path,
fraction), so these updates are atomic and insertions before the current leaf
do not shift playback to another item. Numeric indices are derived outputs for
the current tree, not retained selection identity. Missing cursor targets fall
back to the initial position, restricted to the visible range.

`puri-widgets::slider` supplies continuous interaction and painting;
`range_slider` supplies discrete-range interaction and painting; `tree_slider`
composes their descriptions and owns the pure hierarchy/range logic, generic
over caller-supplied ordered occurrence keys. The editor adapter supplies paths
through the actual GID list positions and writes caller-owned view state. None knows
operations, tools, orientations, or CAM. Controls are emitted in evaluation order.

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

The controls function, view function, and preview projection all run again on
each projected frame; controls do not cache emitted widgets. A surrounding
`render` declaration may reuse its pure evaluation through the general
dependency-tracked computation system, as the CAM example does when constructing
its program tree. The `with controls` constructor declares its reads tracked;
it only evaluates arguments and returns data. Effects or untracked reads in
those arguments still prevent reuse. The view's expensive geometry also uses
that system.
Controls do not reduce the view's assigned size. Their bottom alignment comes
from their actual measured height and the settled view bounds, not a matching
constant in the example.
