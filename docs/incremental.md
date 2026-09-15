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
    Ready(Arc<T>),
}
```

Pending is scheduling state, not an absent or an evaluator error. `T` can itself
contain normally returned absents. Interrupted computations remain runtime errors,
and catching them makes a parent non-reusable just as for synchronous reads.
Consumers choose whether to show a previous value, a placeholder, or some other
representation; the scheduler never silently presents a stale value as ready.

Each node has at most one running job and one latest replacement. Changing the
prepared snapshot cancels the old request and replaces any queued request. The
executor bounds concurrent jobs and requeues replacements so other nodes can run.
Completion carries a generation; the owner validates current prepared inputs
before publishing it. Obsolete completions cannot overwrite newer state. Worker
panics are transported and resumed on the owner thread, not converted into an
eternal pending state. Cooperative cancellation calls registered callbacks once,
allowing a native library's cancellation token to be connected directly.

Workers notify a caller-supplied wake function. Between graph reads, `Tasks::poll`
imports notifications and advances the graph revision; reads then publish current
completions and invalidate dependents through the usual dependency mechanism.
Once a request has been observed as pending, later reads in that same revision
also see pending, even if the worker has finished. A new request may complete
inline before its first observation. This keeps shared consumers consistent and
prevents a completion from losing the wake that should refresh earlier readers.
There is no timer polling loop. Dropping a node or its task owner cancels work;
replacing a document starts a fresh graph with the same executor/wake capability.

Native editor windows use a single background worker each. An application event
imports completions and rebuilds the normal frame. Web and default headless
contexts explicitly use the inline executor for now. Tests can supply a controlled
queue or the threaded executor without a windowing harness.

## CAM integration

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

This is the first integration. Ordinary Fidget mesh/voxel previews, IoP drawing,
completions, and other UI projections are not converted.

## Deferred

Grap path generation and image rasterization/readback remain synchronous. There
is no browser-worker executor, progressive quality scheduler, durability tier,
or Grap-language memo/async syntax yet. Incremental lambda calculus and
incremental Fidget algorithms remain separate research.
