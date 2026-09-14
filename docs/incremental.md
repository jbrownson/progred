# Dependency-tracked computations

`incremental` is a synchronous, caller-owned graph, independent of GID, Grap,
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
the graph. Each node retains only its latest result, not slider history.

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

## CAM integration

The mesh toolpath viewport composes nested nodes:

1. Run the Grap generator into a native recording and evaluation result.
2. Select solid/playback settings independently of path appearance, then build
   the remaining-stock Fidget expression from the recording.
3. Mesh that expression at the requested depth.
4. Separately interpret the recording into path/tool triangles with playback
   and appearance settings, then combine the two geometry layers.

Camera, viewport size, and display scale are outside geometry generation. Orbit,
zoom, and resize still build the whole UI and render a new image but reuse valid
geometry. Playback changes retain the recording; depth changes also retain the
stock expression. Path color or line thickness changes retain both the stock
expression and its mesh. Program edits invalidate observed reads. Invalid
programs show their absence rather than a stale successful image. Native mesh/recording values
never travel through Grap as opaque values.

This is the first integration. Ordinary Fidget mesh/voxel previews, IoP drawing,
completions, and other UI projections are not converted.

## Deferred

Cold loads and geometry edits still block. There is no async queue, progressive
quality scheduler, durability tier, or Grap-language memo syntax. Cancellation is
checked at graph boundaries and optionally inside recipes; it does not yet stop a
running Fidget mesher. Async needs immutable job inputs, cooperative cancellation,
latest-request coalescing, and generation-checked publication. Incremental lambda
calculus and incremental Fidget algorithms remain separate research.
