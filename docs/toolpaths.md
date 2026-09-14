# Toolpaths

The toolpath library is an experimental path-geometry layer, not a machine
program or a collision/clearance check. It adds no evaluator or editor-core
cases and requires no new dependencies.

## Generation and interpretation

Rust generators call `paths::Sink::start_at` and `line_to` with finite 3D
coordinates. A new start begins a separate path: it is neither a cutting link
nor a rapid move from the previous endpoint. A line requires a preceding start.
Calling generators in sequence composes their output. `MapPoints` adapts another
sink, so translation, reflection, surface mapping, and further adapters compose
without recording intermediate paths.

Tests use `Recording` as an optional initial representation: a native vector of
start/line commands, with replay into any sink. The line preview instead consumes
emitted points directly into one projected vector path and its bounds; the 3D
voxel preview consumes them into native Fidget fields, while the mesh preview
emits tube vertices and indices directly. There is no
intermediate command recording, GID command list, or opaque Rust program passed
through Grap. Nothing is memoized across frames.

Grap uses ordinary calls to scoped `start at`, `line to`, and
`map points` functions. `do` supplies sequencing; lambdas supply reusable
generators. Emitters outside an installed output scope return `toolpath output
required`. `map points` takes a callable under `mapping` and an unevaluated
`expression`. For each emitted point, it calls the mapping with `x`, `y`, `z`;
the mapping returns an ordinary record with those fields (the `point` function
constructs one). Nested mappings run inside-out and restore the surrounding
mapping on exit, including on failure. Mapping functions are intended to
compute coordinates, not emit more paths.

The scoped output owns its mapping chain and sink. No mutable state is held by
the library between evaluations. Each emitted sample consumes evaluator fuel,
including samples generated inside Rust. Invalid commands return ordinary absents;
`do` stops at the first one. An enclosing `match` can recover and continue emitting,
with earlier output retained. No failure latch can override that recovery.
A consumer needing atomic results must stage its sink and discard it on a failed
final result; the preview does. Native generators propagate their sink's errors
immediately.

## First example

The example's Grap `diagonal passes` and `sample pass` functions follow
`ToolPathHelpers.DiagonalUVs` in
`rhino-cube-plugin/src/models/Paths/ToolPath.cs`. It distributes nondegenerate
interior diagonals across the unit square and samples each including both
endpoints. Its maximum sample spacing is in **UV coordinates**, before mapping;
it is not a world-space tolerance, stepover, or scallop-height guarantee.
The row loop and point loop use the ordinary `iterate` function, emitting
`start at` and `line to` calls without collecting intermediate lists. Sampling
policy is document code, not a toolpath FFI. General `min`, `max`, `ceil`,
`hypot`, and `is finite` operations belong to the f64 library. The example
rejects nonfinite or fractional row counts and nonpositive/nonfinite spacing;
these checks are a flat sequence of ordinary `require` guards. Loops explicitly
return the list library's `iteration finished` absent at their endpoint rather
than relying on a failed match. An excessive sampling request is bounded by
evaluator fuel.

The example maps two crossing sweeps to the top quadratic patch used by the
Rhino cube (size 1, chamfer 0.1, center-control-point displacement 0.5):

```
x = 0.8(u − 0.5)
y = 0.8(v − 0.5)
z = 0.5 − 2u(1 − u)v(1 − v)
```

This corresponds to a face-center depression of 0.125. The second sweep reflects
the unit-square y coordinate before the same surface mapping. Unlike Rhino's
finishing program, this example does not reverse row order for that second
sweep, compensate for the ball radius/tool orientation, or construct linking
curves. Those are separate future combinators. The first fixture covers one
face only and retains the original model units without assigning a machine unit.

## Previews

`preview paths` is an opt-in viewport function with `value` (a callable program),
`width`, `height`, and optional `fuel` (otherwise Grap's default). It returns an
ordinary preview declaration containing that program and these parameters. The
native widget runs the program with this explicit budget. It fits a fixed
isometric projection inside the assigned pane, with a 16-logical-unit margin,
reduced in tiny panes.
All paths are green; there is no inferred cut/link classification.

`preview paths 3d` combines `value` (an ordinary Fidget field or colored scene)
with `program` (a callable path generator). It also takes an explicit positive
f64 `line radius` and opaque `color`, plus the same width, height, and bounds
as Fidget's `preview 3d` and an optional path-evaluation `fuel` budget. Its result
is an ordinary preview declaration, not an opaque native value.

The native sink constructs a capsule for each line and unions the segments of
each continuous path into its own Fidget scene object. `start at` separates
objects without adding a connecting move. A zero-length line is a sphere; an
isolated start draws nothing. Points convert to Fidget's f32 coordinates, with nonfinite
or out-of-range inputs rejected. The preview uses the existing Fidget camera,
depth testing, colors, and lighting. Paths precede the model for exact depth
ties; there is no screen-space overlay or displaced geometry. `line radius`
is visual thickness in model units, **not a cutter radius**. A failed final
program result discards the entire preview, including partially emitted paths.

`preview paths mesh` takes the same arguments, plus Fidget's u64 `mesh depth`
(1 through 8, default 6). It meshes only the model's implicit fields. The path
sink emits an indexed capsule mesh for each segment directly: twelve sides and
three latitude intervals per round cap. Segments overlap at joints; they are
visual tubes, not a boolean-unioned or manufacturing-ready solid. Zero-length
lines draw spheres and isolated starts draw nothing. No Fidget expression or
meshing step is involved in the path geometry. Paths and model share the triangle
renderer, camera, lighting, and depth buffer; no overlay or depth bias is used.
Failed path generation discards the whole preview before meshing the model.

Command+9's example uses this mesh viewport at depth 5: 42 gold paths (1,056
segments) over a blue cube, with a 100,000-fuel budget. Every frame reruns the
Grap path program, generates the tubes, and remeshes the cube. There is no mesh
or image cache. Native builds use GPU triangle drawing with synchronous readback;
web/headless fallback uses the same geometry in the CPU triangle renderer.
Expand `panes` to change `mesh depth` or replace `preview paths mesh` with
`preview paths 3d` for comparison. Drag to orbit and scroll to zoom. The document
contains its own copy of the cube definition so it is self-contained. Its
surface mapping and cube parameters are independently editable; changing one
does not automatically change the other.

In the voxel preview, many sampled segments produce much larger Fidget expressions
than the model alone. This is a first static visualization, not a performance claim for large
machine programs. The opt-in toolpath orbit canary in
[performance checks](performance.md) exercises the real document and preview.
The initial single-field version's headless sandbox check (2026-09-13, CPU
fallback, 800 × 1200 raster) took a median 1.60 seconds across three measured
frames after five warm-up frames.
That is not a measurement of interactive native GPU performance.

The initial single-field version failed on the native GPU backend: a 2026-09-13
trace at 599 × 1280 × 640 measured 1.4–1.9 seconds per render and zero path
pixels before depth merging. Its VM tape contains 6,736 register-spill
load/store operations. The pinned Fidget shader's `OP_MEM` handler is explicitly
unimplemented and returns without a result, although `RenderShape::new` accepts
the bytecode. The CPU backend supports these operations. CPU screenshots and
successful bytecode construction therefore do not establish native rendering
support. Keeping each continuous path separate avoids spilling in this example;
the regression test checks every path for load/store instructions. A sufficiently
large individual path can still encounter this upstream limitation. There is no
spill emulation or automatic CPU fallback for it. Before shipping, follow up
upstream on implementing or rejecting these instructions; no issue has been filed
yet. The user confirmed that the split version renders the paths and accepts
orbit drags, but remains choppy. A subsequent native trace of 50 renders at
599 × 1280 × 640 averaged 234 ms (196–279 ms), with 215 ms on average in
wait/readback. This is rendering time, not a complete input-to-display latency
measurement. The [cube meshing experiment](fidget-meshing-2026-09-13.md) motivated
the current mesh viewport, which bypasses the GPU VM entirely and does not yet
retain geometry across frames.

Cutter playback, orientation, radius compensation, explicit links, stock
removal, and machine/postprocessor output remain separate next steps. In
particular, the existing Fidget cube field is not an exact signed distance;
subtracting a cutter radius from it would not implement a geometric offset.
