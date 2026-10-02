# Deferred work

These are open questions carried forward from the review, not tasks scheduled
for the next cleanup. The current implementation is documented in
[model.md](model.md), [projections.md](projections.md), and [puri.md](puri.md).

## Active insertion hidden by a specialized projection

A specialized projection can omit the structure where an insertion is pending.
The general interaction and presentation policy needs dedicated design work.
Do not hide a fallback policy in reusable widgets or infer it by inspecting a
render tree. This was explicitly set aside for another day.

The shared list/record combinators preserve immediate pending children. Compact
Grap lambdas, cases, binding clauses, and control/operator calls now decline when
they cannot show an active insertion. That fixes those specific projections;
it is not a general fallback policy for arbitrary facets such as line controls.

## Discoverability of expression definitions

Keep Grap expression references shallow, including the source shown by
`{evaluate: expression}`. The toolpath example places an ordinary reference to
the ball-tool cell immediately before its evaluation so the normal projection
exposes its editable contents. Both occurrences reference the same definition;
neither is a special declaration. Revisit navigation or expansion affordances
if finding these definitions becomes a recurring problem, rather than changing
`evaluate`'s projection now.

## Selection destinations and history

Plain selections now work with line controls' missing-state defaults, so paste,
deletion, and undo do not need editor-initialization hooks. Destination policies
may still merit customization: deletion chooses a surviving sibling or parent;
undo restores an edge location, not its prior caret or pending query. Preserving
those states is a separate UX decision, not necessary to make restored selections
usable. Never resurrect active drags or IME composition from history.

Line-editor conversion runs after every accepted event, including caret
movement; equal results already skip document writes. Avoiding unnecessary
conversion is a separate optimization. Projections must not rely on that
optimization to keep a missing value absent: a direct line control offers its
spelling for write-through. Lambda names instead use the completion picker,
which stages input until commit.

## Duplicate-definition inspection

Normal lookup selects the document definition, otherwise the first library
definition in load order. Duplicate definitions are tolerated, not an intended
composition mechanism. Later we may show an indicator and an inspector for the
other sources; no such UI is implemented now. Explicit source-qualified paths
remain available internally.

## Editor-authored libraries

A document can declare libraries that draw its own records (the root's
`libraries` field; see projections.md). What remains for a complete
editor-authored library: completion offers, so a library can construct its
values; library-scoped definitions and names, beyond ordinary document cells;
and loading a library defined in one document into another, as an explicit
editor operation with its configuration and definitions reachable from root.
The proposed descriptor lists cell references, not a second record duplicating
their definitions; loading needs that descriptor and a source that can resolve
the listed cells, which a document can provide without becoming part of the
library's semantic type.

## Quotes that fill bound variables

A quote fills only its `unquote` holes; a bare reference in a template stays
data. The preferred direction is for a quote to also fill every variable bound
around it, keeping `unquote` for computed expressions, so a template reads as
data with names: `"{circle: {x: x, y: 50}}`. It waits on two idioms that rely on
bare references staying data, both workarounds worth removing first:

- The CAM example writes deferred passes as closures in their stored form,
  `"{closure: {params, body, environment}}`, whose `body` refers to variables
  that `environment` captures through `unquote`. A lambda would capture its
  whole lexical environment. The original reason, not retaining recursive
  list-mapping accumulators, went away with scoped tree emission. Lambdas that
  capture only the variables their body uses would let these be ordinary lambdas.
- Keyed controls (`slider`, `radio`, `tree range`, `tree cursor`, and
  `tree program cursor`) take their state key as an evaluated argument, so
  programs quote a cell to pass it. The CAM's `control key: "preview mode` reuses
  the binder of the variable that holds the radio's value, and that cell is also
  a record key and a pattern binder in the view. The forest's controls receive
  their state and an update callable and need no key; giving `radio` and
  `tree program cursor` that form and retiring the keyed capabilities removes
  the idiom.

An audit of every document found 25 bare references to bound variables inside
templates, all in `examples/toolpaths.gid` and `website/public/lessons/shape.gid`,
which shares its code. Of the 198 holes across all documents, 122 are plain
variables and 76 are computed expressions, so `unquote` stays common in either
design. Independently of the semantics, the picker could offer in-scope variables
inside a template and insert them as holes.

## Address lifetime

Paths and source traces already preserve definition source. A future identity
review should specify what survives list moves, deletion/undo, definition
replacement, and transient-root changes. A cell has meaning in a resolution
context; neither a path nor a cell-relative trace is a global address.
Keep occurrence identity, cell identity, definition source, and view identity
distinct while answering those questions.

## Completion ranking

Revisit how library-provided offers rank alongside built-in constructors,
cell references, and interpretations of the query. The universal list currently
groups library offers together; with an empty query, `new lambda` appears among
number literals because of library order. Providers cannot express that it
belongs with other constructors. Design that distinction through the completion
interface rather than special-casing lambda or particular numeric libraries.
This is deferred UX design, not a priority for the current cleanup.

## Reporting completion failures

Offer construction decides applicability; activating an offered completion
consumes the input. Its Grap continuation can still fail at runtime. For now,
unsuccessful preparation installs neither the insertion nor its staged
selection/annotation changes. Declining after an effect is now an evaluator
contract error and prints to stderr; other completion failures remain silent.

Later, design an editor-owned way to surface these failures, such as alerts or
notifications, using the failure details in GID values. Do not add that machinery
now. Explicit decline is distinct from ordinary absent results, which are
definitive values and retain their effects. The eventual reporting policy must
not assume every absent warrants an alert.

## General computation reuse

The former canvas memo observed less than evaluation could use. It remains
removed; canvas programs record once per visible frame. The new general
[computation graph](incremental.md) first serves CAM geometry and observes selected
definitions, missing lookups, native implementations, and declared FFI inputs.
Native CAM stock jobs now use the graph's generic background boundary; implicit
CAM jobs publish coarse-to-fine images through its progressive-result interface.
Browser workers, Grap-language memo/async boundaries, user-facing quality controls, durability
tiers, and further integrations are deferred. Future readers of ordered contributor sets must
observe that set, not just a selected definition. Do not add an ad-hoc cache
or event-specific relevance checks alongside this mechanism.

The [2026-09-04 tree profile](tree-profile-2026-09-04.md) records measurements
from that implementation; timings are historical observations, not guarantees.

### Lowered results across evaluator boundaries

Public evaluation/application APIs return owned `RuntimeValue`; code and
lexical environments have shared ownership, while execution caches and host
capabilities remain evaluation-local. The projection/presentation/controls/drawing
chain and retained event handlers now preserve runtime callbacks. Inline origins
are anchored to their original occurrence. The
`website_forest_hover_finds_the_available_call` regression is enabled and passes.

Data-oriented projections and native functions still have explicit GID adapters;
these can materialize runtime containers even when a later runtime-aware
projection ultimately handles them. Reducing that conversion work is a remaining
optimization. Closure code origins now round-trip as explicit GID metadata;
keeping values lowered is an efficiency choice, not the only way to preserve
their source annotations. Do not preserve a creator's dynamic stack as a caller.
Fold classification, collapsed display, source highlighting, text/blob/color
partials, empty partials, expanded structural lists/records, control-form
partials, shallow Grap reference/declaration helpers, general call/lambda/value/FFI
projections, and numeric facets/notation now avoid requesting a whole GID view.
Domain and other legacy projections still need migration. Callable parameter
discovery and the `evaluate`/`render` source interfaces now retain runtime values,
including embedded closures through memoized evaluation. Explicitly expanding
a native closure's representation still materializes its body/environment. Computed
`match` cases and `let`/`where` bindings retain their runtime lists and executable
children; individual patterns still adapt to the GID-oriented matcher, and
repeated-binder equality still compares materialized data. Document edits and
copying are intentional GID boundaries.
Scoped layout execution, nested child results, and the `drawing` constructor now
preserve runtime values, including callbacks in absence details. Layout picking
materializes only on activation. Border-projection composition and declaration
application now retain runtime callables and results too. Narrow path, paint,
and drawing-command parsers remain GID-oriented, as does the viewport preparation
memo's input (currently read directly from the stored declaration).
Toolpath sequence/mapping execution and the 2D path preview now retain runtime
callbacks and results. The tree collector and its memo/control handoff now do
likewise, comparing runtime inputs/results with source-aware equality. The 3D
preview/recording pipeline also retains executable programs; model/playback/
appearance decoders and render-failure outcomes still adapt their data to GID.
Radio options and the older list-based range-control inputs remain GID-oriented.
The [owned-result experiment](performance.md#owned-result-experiment--2026-09-17)
measured an earlier, removed implementation. Its results motivate profiling
the new consumers, not assuming that keeping runtime values is always faster.

The [2026-09-23 comparison](performance.md#runtime-callback-handoffs--2026-09-23)
covers the pre-ownership baseline, the ownership checkpoint, and the consumer
migration. The newer [tree-to-3D comparison](performance.md#runtime-tree-to-3d-pipeline--2026-09-23)
improves uncached construction; steady-state controls frames remain roughly at
the ownership checkpoint. Profile remaining conversions before broadening
the migration. Exclude compilation and isolate build outputs for each source
tree: sharing a target directory between archived copies reused incompatible
artifacts.

`with controls` still returns a declaration interpreted by its partial. Direct
widget emission remains separate, deferred work. Profile uncached construction
as well as memo hits before changing that boundary.

### Fidget refinement and sharp edges

When we next examine Fidget internals for Progred's needs, revisit the
[analytic tool-sweep experiment](tool-sweep-experiment.md). Its point signs pass
the sampled checks, but meshing yields invalid vertices and is slower than the
working approximation. Investigate stable gradients and interval behavior then,
alongside incremental/progressive computation; do not turn the current tool
profile work into a general analytic-sweep research project. This is not yet an
identified upstream bug. Tool geometry no longer carries approximation tolerance.

Future upstream investigation may cover reuse across progressive resolutions or
model edits, and more accurate surface-hit/normal evaluation at sharp edges.
Current raster refinement recomputes each level; finer depth sampling reduces
both missed thin intersections and wrong-face normals but does not eliminate
them. See the [tool-rim diagnostics](performance.md#implicit-stock-quality-and-cancellation-investigation--2026-09-14).
Command+4 now uses a retained mesh as the immediate camera-dependent fallback
while current implicit images refine it. That removes the need to finish an
obsolete coarse implicit image just to provide orbit feedback. Standalone
implicit previews still cancel even their first stage on new input. Revisit
finish-coarse scheduling only if a consumer without a usable fallback needs it;
obsolete results must never become the current computation's result. Skipping
intermediate meshes during rapid geometry edits is another possible policy,
with the tradeoff of an older mesh when the user next orbits.

## Report Vello image-atlas flashing

Prepare an upstream bug report for the reproducible Vello 0.9.0 image omissions
documented in the [wide-preview investigation](graphics-memory-2026-09-16.md#wide-preview-flashing-follow-up-2026-09-18).
Repeated 4200 × 2800 image replacements drop every third image while 4000 × 2800
replacements work. The atlas protects obsolete recent images and silently omits
the current image when allocation fails. Reusing a registered texture works at
fixed size, but unregistering/replacing it during resizing reproduces the failure.

Before filing, check for an existing upstream issue and reproduce against the
then-current release with a minimal standalone example, including version,
backend, physical image dimensions, and expected/actual pixel readbacks. No
report has been filed. The owner prefers avoiding local Vello patches. The
[separate compositor](vello-compositor-experiment-2026-09-18.md) now bypasses the
atlas for native draw-image operations, with per-window upload ownership and
headless fidelity, resize, lifetime, and performance checks. Image brushes still
use Vello's atlas. Direct mesh drawing, including replacing partial implicit-image
coverage over a GPU mesh fallback, remains a separate step; the mesh pipeline
still performs readback.

## CAM preview navigation and execution grouping

Explore an execution hierarchy distinct from the document's authoring outline.
The proposed meaning of an operation is a boundary where a person or robot
intervenes: flipping the part, re-probing, or similar work. Within it, indexed
orientations, tool uses, and logical groups of cuts are possible levels; their
ordering and names are not settled. Reusable tools and orientations need not
be owned by that tree: execution occurrences can reference the same definitions
in several places.

A first version of the notched range controls now follows ordinary nested
program lists, with coarse groups at the bottom and continuous playback at the
top. It uses equal-width notches without permanent names; Cmd-hover source
attribution remains a useful next step. Adjusting a coarser row resets every
finer row to `All`, except when clicking its amber-marked current item, which
preserves finer selections. Playback markers identify the current section in each row.
`All` includes future items; explicit bounds include insertions between their
endpoints. Uneven branches align from the fine-grained end. Playback retains its leaf-path cursor
when it remains in the selected range. Producers that reconstruct lists can
still change their positions; preserving correspondence through Grap list
construction is separate work, not solved by inventing identities in the widget.
Prior cuts remain in stock history.
See [controls](controls.md) and [toolpaths](toolpaths.md). This does not impose
an operation/orientation/tool taxonomy on programs.

## Keyboard navigation and list reordering

Ordinary arrows now use [layout-derived navigation](navigation.md). Placeholder
Tab navigation, grow-selection commands, directional history, and moving a list
item with a modifier-plus-arrow shortcut remain separate decisions. The range
controls also remain pointer/touch-only.

## Website as an interactive explanation

The owner has registered `prog.red`. Consider developing its presentation
alongside editor UX: a section for each of Progred's central ideas, with an
embedded editor demonstrating that idea. This is a proposed direction, not a
website implementation or hosting decision.

## CAM machine-axis alignment

Revisit when generating G-code: prefer indexed orientations that let an
individual knurling pass use only two linear axes, where the actual compensated
trajectory and machine kinematics allow it. Surface-contact diagonals are
planar, but normal-offset ball-center trajectories need not be. A rotated
programming coordinate system alone does not imply fewer physical axes move.
Do not flatten trajectories or alter geometry in the postprocessor without an
explicit accuracy policy. Setup-up and pull/climb ordering are already explicit
in the Grap example; machine rotary solutions and fixture clearance are not.

## Hover traversal order

Revisit when the document/widget mix includes more expensive overlapping hover
targets, especially canvases covered by opaque popups or other content. Current
placement runs probes in painting order; foreground claims replace background
claims, but cannot avoid the work of background probes already run.

Compare this streaming pass with retaining probes and querying topmost-first,
where a direct hit or occlusion can skip probes underneath. Measure skipped
hit-test work against the extra allocations, retained captures, and traversal;
include ordinary source documents as well as overlap-heavy cases. Preserve
direct-over-extended claim priority, clipping, and nested-floater ordering, and
check identical hover, paint, and dispatch results. This is a deferred experiment,
not a decision to reverse traversal or add caching.

## Text input beyond plain typing

Native macOS IME is enabled while a text field is edited; dead keys and
interrupted composition were checked by hand on 2026-09-29. Left open, to take
up only when text entry becomes a focus:

- The macOS emoji and symbols picker inserts nothing. winit 0.30 forwards an
  `insertText` only while composition text is marked, and the picker inserts
  directly; opening it may also read as focus loss, which clears the edit.
  Needs a winit change first.
- The browser host enables no IME. Its keys arrive through the page's own
  keyboard bridge, so composition would need browser composition events.
- CJK composition and candidate placement, Linux IME, and AltGr characters in
  Windows browsers are implemented but unverified.

## Platform work

### Fidget constant fields on the GPU — verify adopted upstream fix

Fidget 0.5.0's GPU `RenderShape` allocates its variable buffer from the compiled
variable count. A constant field therefore creates a zero-byte buffer and tries
to bind it as storage. Creating a zero-sized WebGPU buffer is allowed; binding
a zero-sized range is not. This is platform-independent validation, not a Metal
quirk; see [WebGPU buffer creation](https://gpuweb.github.io/gpuweb/#buffer-creation)
and [bind-group validation](https://gpuweb.github.io/gpuweb/#dom-gpudevice-createbindgroup).

Upstream checked on 2026-09-05 at main commit
[`94123ea`](https://github.com/mkeeter/fidget/blob/94123ea4a351fedf66df404c31f3fed0e1474b0e/fidget-wgpu/src/lib.rs#L280-L293):
this is already fixed by allocating at least four bytes. The change landed on
2026-08-25 in [PR #461](https://github.com/mkeeter/fidget/pull/461), commit
[`19a9049`](https://github.com/mkeeter/fidget/commit/19a904926a72a4e8509d02060bfef109fe37e488).
Searches of open/closed issues and PRs for constants, buffers, and bindings found
no separate matching bug report. The older [PR #318](https://github.com/mkeeter/fidget/pull/318)
fixes a different constant-evaluation problem. Do not file a duplicate report
for the allocation/binding defect already fixed in #461.

The 2026-09-13 dependency update adopts this fix through the reviewed upstream
revision `0c89e87e1b3a6d15cc0976ab6ff05a09f9cf91d6`. There is no constant-field
workaround. The ordinary CPU backend remains available when no GPU is available
and on the web; its tests do not verify GPU behavior.

Still test zero, positive, negative, and computed constant fields on the real
GPU. The ignored `gpu_colors_constants_and_workspace_reuse` test exercises
these and multi-object colors without allowing CPU fallback, but its attempted
run inside Seatbelt found no Metal adapter. Normal CPU tests pass.
If validation still fails, prepare a minimal standalone Fidget reproducer and
capture the exact validation error, dependency versions, OS, and adapter/backend
for an upstream report. The original Progred crash did not preserve the GPU
validation message, so its precise cause remains inferred from this code path.
Metal itself is unavailable inside the build sandbox. Do not forbid constant
fields in the Fidget language or merely hide their completions.

### Additional ports

The iPad feasibility host was removed while macOS/Linux mature; its notes are in
[history](history/ipad-host.md). Older visionOS and language-extension ideas remain
uncommitted possibilities in the historical notes.
