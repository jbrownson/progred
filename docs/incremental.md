# Dependency-tracked computations

`incremental` is a caller-owned graph, independent of GID, Grap,
Puri, and the editor. It retains explicit subcomputations, not frames or
event-specific decisions about whether to rebuild the UI.

```rust
let runtime = incremental::Runtime::default();
let input = runtime.input(2i32);
let square = runtime.memo({
    let input = input.clone();
    move |read| Ok(input.read(read).pow(2))
});
let result = runtime.memo(move |read| Ok(*square.read(read)? + 1));
assert_eq!(*runtime.read(&result)?, 5);
input.set(3);
assert_eq!(*runtime.read(&result)?, 10);
```

## Dependencies and lifetime

An `Input<T>` owns a value and its change revision. Setting an unequal value
advances the runtime revision. A `Memo<T>` owns one recipe, its latest result,
and the inputs/children that recipe actually read. A cached child is still a
dependency. Re-execution replaces dependencies, removing abandoned branches.

Pulling a root validates dependencies in observation order, stopping at the first
change. Equal results keep their previous change revision and shared result,
preventing unnecessary downstream work. `memo_by` accepts an equivalence function;
mesh nodes deliberately report unequal on re-execution instead of comparing
large vertex buffers. There is no reverse-edge invalidation queue or hashing.

Recipes read changing inputs through `Read`; captured props must be immutable.
`Read::untracked` disables reuse, including in consumers. Input writes during
computation, cross-runtime dependencies, cancellation, and cycles return errors.
Interrupted work is never published. If a recipe catches a failed child read,
its fallback is non-reusable: the incomplete read did not establish a dependency
that could validate that fallback. A failure during dependency validation reruns
the recipe so its own recovery logic can handle the failure. `Source` projects
keyed observations from an immutable snapshot, so unrelated snapshot edits need
not invalidate a reader.
Missing results are observations too.

Handles identify computations explicitly; no execution path is guessed. The
editor retains library-owned typed roots under its existing view identity and
source-qualified location. Different viewports have independent roots. Unused
roots are released after a pass without demand; replacing the document drops
the graph. A synchronous node retains its latest result, not slider history.
An asynchronous node can also retain that result while its replacement runs.

## Grap and foreign calls

`grap::memo` observes whole evaluations, using a fresh interpreter on a miss so
prepared calls cannot bypass another memo's observations. This is a Rust API,
**not yet a Grap `memo` function or arbitrary-expression intrinsic**. Native nodes
can nest evaluations and other native nodes in the same graph.

The editor supplies a document/ordered-library snapshot. Cell reads and calls
observe the selected source and definition, including missing definitions and
native implementation identity. Reorder/load/unload and document overrides are
visible when they change resolution. A future computation enumerating contributors
or completions must observe that ordered result, not just the selected definition.
Those UI computations are not memoized here. Expressions, arguments/environments,
and fuel belong in explicit node inputs.

Foreign calls are untracked by default and prevent reuse. `tracked()` opts a
function or scoped overlay into an author contract: captured state is immutable
or evaluation-local; changing external reads use `Context::read(&Input<T>)`;
observable writes use `Context::effect`. The input owner calls `set` when the
resource changes. This is not discovery of arbitrary Rust closure reads.

Ordinary effects prevent reuse: a hit must not omit a write.
`grap::memo::with_recorded_effects` declares that the caller owns and returns
**all** effects, such as toolpath commands; it does not itself record anything.
It must not wrap unrecorded writes into an enclosing editor or drawing
sink. Unknown foreign calls still prevent reuse there. Evaluator halts are not
reused; normally returned absents are ordinary retained results, even if their
reason names cancellation or another halt. Completion status, not the reason,
determines whether execution finished.

## Background computations

`background::Tasks::memo` connects a tracked preparation memo to an owned worker
computation. Preparation reads the graph on its owner thread and returns an
immutable, `Clone + Send` snapshot. The worker receives only that snapshot and a
`Cancellation`; it cannot read the editor or the graph. Its function and captures
must be thread-safe and must not perform unrecorded external effects. This is a
split-phase native API, not an arbitrary Grap closure moved to another thread.

The returned `AsyncMemo` is an ordinary dependency of other memos. Its read is
non-blocking with a queued executor:

```rust
enum Availability<T> {
    Pending { previous: Option<Arc<T>> },
    Refining(Arc<T>),
    Ready(Arc<T>),
}
```

Pending is scheduling state, not an absent or an evaluator error. `T` can itself
contain normally returned absents. Interrupted computations remain runtime errors,
and catching them makes a parent non-reusable just as for synchronous reads.
Consumers choose whether to show a previous value, a placeholder, or some other
representation; the scheduler never silently presents a stale value as ready.

`Tasks::memo_progressive` supplies the worker an additional `publish(T)` callback.
Each publication is a usable result for the **current** request, reported as
`Refining`; returning the final result changes it to `Ready`. A new request
retains the last published result as `Pending.previous`. Intermediate values use
the same generation checks, owner-thread polling, and dependency invalidation as
final results. Several reports arriving before a poll collapse to the latest.
A final failure remains a failure, not a successful intermediate result.
The publication callback is `Send`; a parallel computation may lend it to
scoped workers under a mutex. Calls remain serialized, and it cannot outlive
the job. This adds no task, queue, or scheduling policy.
`memo` is the same mechanism without intermediate publications. The worker, not
the graph runtime, decides what refinement means and which quality levels to run.

`memo_reporting` additionally supplies a thread-safe work-progress callback;
`AsyncMemo::progress` observes its latest completed/total counts through the same
graph revision and generation checks. Counts do not make a result ready or
replace an intermediate value. They reset on a new request and disappear on
completion. A worker may reset them for a new stage. These are work units, not
an estimate of elapsed or remaining time; the producer owns reporting frequency.

`memo_reporting_when_ready` accepts preparation returning `Option<I>`. `None`
means a dependency is not ready: it cancels old work, clears any queued
replacement and progress, and retains the last result as `Pending.previous`.
It submits no waiting/no-op job. When preparation becomes `Some(input)`, the
ordinary dependency graph starts the job. Waiting is not a failure or an absent;
old completions remain subject to generation checks. The always-ready APIs wrap
their prepared inputs in `Some`.

Each node has at most one running job and one latest replacement. Changing the
prepared snapshot cancels the old request and replaces any queued request. The
executor bounds concurrent jobs and requeues replacements so other nodes can run.
Each report carries a generation; the owner validates current prepared inputs
before publishing it. Obsolete reports cannot overwrite newer state. Worker
panics are transported and resumed on the owner thread, not converted into an
eternal pending state. Cooperative cancellation calls registered callbacks once,
allowing a native library's cancellation token to be connected directly.

Workers notify a caller-supplied wake function. Between graph reads, `Tasks::poll`
imports notifications and advances the graph revision; reads then publish current
reports and invalidate dependents through the usual dependency mechanism.
Once availability has been observed, later reads in that same revision
see the same value, even if the worker has progressed. A new request may complete
inline before its first observation. This keeps shared consumers consistent and
prevents a completion from losing the wake that should refresh earlier readers.
There is no timer polling loop. Dropping a node or its task owner cancels work;
replacing a document starts a fresh graph with the same executor/wake capability.

Native editor windows use a single background worker each. An application event
imports completions and rebuilds the normal frame. Web and default headless
contexts explicitly use the inline executor for now. Tests can supply a controlled
queue or the threaded executor without a windowing harness.

## CAM integration

A pane declaration can apply an optional `prepare` function to its raw `value`
data through a tracked application memo before `viewport` receives the result
and dimensions. The CAM example instead uses the controls' `tree program cursor`:
it memoizes a final-encoded builder's items and explicit source-linked hierarchy,
consumes the hierarchy directly, and returns the items to the preview. Its explicit inputs are the
builder callable and fuel; observed definitions supply the dependencies. Size is
not captured by the cutting functions, so resize does not invalidate the
recording upstream of geometry generation. See [trees](trees.md).

The mesh toolpath viewport composes nested nodes:

1. Run the Grap generator into a native recording and evaluation result.
2. Prepare a worker snapshot: shared recording, solid/playback settings, and mesh
   depth. Path appearance is not an input to this job.
3. In the worker, construct the remaining-stock Fidget expression and mesh it.
4. Separately interpret the recording into path/tool triangles with playback
   and appearance settings, then combine the two geometry layers.

Camera, viewport size, and display scale are outside geometry generation. Orbit,
zoom, and resize still build the whole UI and render a new image but reuse valid
geometry. Playback and depth changes retain the recording. The worker currently
reconstructs the inexpensive stock expression when either changes. Path color or
line thickness changes retain the stock mesh. Program edits invalidate observed reads. Invalid
programs show their absence rather than a stale successful image. Native mesh/recording values
never travel through Grap as opaque values.

While the worker runs, tool motion and the remaining path update immediately.
The last stock mesh stays visible, desaturated to indicate that it is outdated.
Before any surface is available, the tool/path remain visible with an ellipsis.
The current result restores the original colors. Fidget's supported cancellation
token interrupts octree construction. Compilation and final dual-contour
extraction have only before/after checks; cancellation is cooperative, not a
promise of immediate interruption. Partial meshes are never published.

The implicit CAM `preview paths 3d` uses the same tracked recording and playback
logic, but prepares a camera-dependent image request instead of a mesh request.
The worker constructs the remaining-stock field, interprets future paths and
the cutter as Fidget scene objects, then renders the scene progressively. Camera,
image size, display scale, color and playback are explicit dependencies. Scrolling reuses
the image; camera changes request a replacement without re-running the generator.
The previous **whole image** is dimmed while awaiting the first
current image. Each current refinement replaces it at normal color. An unlabelled
progress bar overlays the top edge of the viewport while work remains.
The bar resets per refinement and disappears on completion. Unlike the mesh preview,
the tool cannot move independently within those pixels. Before any image is available,
the correctly sized viewport shows the empty progress track.

The implicit renderer starts with a maximum edge of 128 physical pixels and
approximately doubles both dimensions on each pass, finishing at the exact
native size, then adds one native-size pass with four times the depth samples.
Every level uses the same camera and model-space render volume; image and depth
sampling become finer together until the final depth-only pass. Pixel rounding
does not change the aspect ratio. Scene compilation is shared within that worker job, but each
voxel raster is independent: coarse pixels are not reused to compute finer ones.
New input cancels the whole sequence and starts a new coarse request, retaining
only the latest replacement in the ordinary async slot. The layout and controls
do not change size as results refine.

The implicit CAM request explicitly uses Fidget's software voxel renderer. It
runs directly on the general background executor, without a GPU-owning thread,
GPU submission, or fallback attempt. The GPU VM cannot execute the stock field's
spill instructions; see [the backend limitation](toolpaths.md#playback).
Cancellation connects to Fidget's voxel cancellation token; the software renderer
uses the JIT on Apple Silicon macOS with its recommended tiles, and the VM with
32/16/8-pixel tiles elsewhere. The local raster patch checks cancellation during
subtile work as well as between root tiles. Expression construction
and compilation have before/after checks rather than immediate interruption.
Obsolete results are discarded by the general scheduler.

The local raster progress callback counts successfully completed root tiles.
The scene sums tile counts across its objects for each refinement and reports
at most every 50 ms, plus stage boundaries, through the ordinary native async job.
The browser's inline executor reports only stage boundaries.
Empty and complex tiles count equally, so this is not a time estimate. Compilation
and image assembly are outside the tile count. The bar is a paint-only reusable
Puri widget, composed as an overlay without changing layout or hover.

### Mesh fallback with implicit refinement

`preview paths refined` (Command+9) composes those native mesh and image recipes
over **one** recorded Grap evaluation. One settings input describes the current
scene, playback, appearance, camera, and image size. A derived memo removes the
camera and image size for mesh generation; equal derived settings retain the
mesh. There is no event classification, orbit flag, inactivity timer, or second
cache.

The current stock mesh is a prerequisite for implicit work. A readiness memo
observes the mesh's pending status; image preparation returns `None` until that
mesh is current. Both nodes are still read while waiting, so obsolete image work
is cancelled rather than left running because the display stopped demanding it.
A current implicit image then wins, including intermediate refinements. While
it is pending, the viewport draws the available mesh synchronously using the
current camera. The old implicit image is not used as the fallback. Current
mesh or image errors remain visible rather than being hidden behind old output.

On camera changes, the mesh remains current. On playback or geometry changes,
the tool/path triangles update immediately and the old stock mesh is desaturated
while its replacement is pending. Both jobs use the existing latest-replacement
queue and cancellation checks. Implicit work cannot overtake replacement meshing,
so orbiting after an implicit result cannot fall back to older stock. Camera-only
changes retain the current mesh and start implicit work without another mesh job.

The mesh is the immediate draft stage. This composition requests a single implicit
pass at native XY resolution with four-times depth sampling. Finished tile batches
replace the mesh only in their explicitly covered regions, including transparent
pixels; unfinished regions remain mesh. The progress bar advances across that
single pass. The raster API also retains the progressive resolution sequence,
still used by the standalone implicit preview. No scheduling policy or domain
types were added to the generic async runtime.

Ordinary Fidget mesh/voxel previews, IoP drawing, completions, and other UI
projections are not converted.

## Deferred

Grap path generation and mesh-preview rasterization/readback remain synchronous.
The implicit CAM image job includes software rasterization. There is no
browser-worker executor, user-facing quality controls, durability tier,
or Grap-language memo/async syntax yet. Incremental lambda calculus and
incremental Fidget algorithms remain separate research.
