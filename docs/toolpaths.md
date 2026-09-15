# Toolpaths

The toolpath library is an experimental path-geometry layer, not a machine
program or a collision/clearance check. Its geometry remains library-owned;
the mesh preview uses the general dependency-tracked computation runtime.

## Generation and interpretation

Rust generators call `paths::Sink::start_at` and `line_to` with finite 3D
coordinates. A new start begins a separate path: it is neither a cutting link
nor a rapid move from the previous endpoint. A line requires a preceding start.
Calling generators in sequence composes their output. `MapPoints` adapts another
sink, so translation, reflection, surface mapping, and further adapters compose
without recording intermediate paths.

`Recording` is an optional initial representation: a native vector of
start/line commands. Playback uses it for total distance and seeking; tests also
replay it into other sinks. The line preview instead consumes
emitted points directly into one projected vector path and its bounds; the 3D
voxel preview consumes them into native Fidget fields. The mesh preview retains
an observed recording, then emits tube vertices and indices from it. There is
no GID command list or opaque Rust program passed through Grap. Final-encoding
generators can still run directly against any sink without recording.

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

The example's cube function owns its size, chamfer, and control-point depth.
It returns an ordinary geometry record with three Grap callables: `field`
constructs the implicit solid, `top face` maps UV points to that surface, and
`normal` gives the outward surface normal at a contact point. They capture the
same dimensions. The implicit field is constructed only when requested; path
generation does not construct or mesh a solid it does not consume. Dimensions
and path arithmetic use f64; `f32 from f64` explicitly rounds the constants at
the Fidget construction boundary.

`crosshatch` takes the top-face mapping. `ball-center passes` obtains it and the
normal function from the cube, then composes surface mapping and ball-radius
compensation. Neither CAM function contains cube dimensions or a duplicate
surface formula. The geometry's analytic normal is still authored alongside its
surface, not automatically differentiated. Tests check both against the actual
implicit field after edits to each cube parameter.

For size `s`, chamfer `c`, and control-point depth `d`, the top patch is:

```
x = (s − 2c)(u − 0.5)
y = (s − 2c)(v − 0.5)
z = s/2 − 4d u(1 − u)v(1 − v)
```

The defaults `s = 1`, `c = 0.1`, `d = 0.5` give a face-center depression of
0.125. The second sweep reflects
the unit-square y coordinate before the same surface mapping. Unlike Rhino's
finishing program, this example does not reverse row order for that second
sweep or construct linking curves. The separate Grap `ball center` mapping
offsets each contact point along the analytic surface normal by `ball radius`;
`ball-center passes` wraps the contact generator with that mapping. The radius
is shared by the mapping and preview tool. The first fixture covers one
face only. The example interprets one model unit as one inch; the runtime still
uses ordinary numeric coordinates, not a unit-aware value type or machine setup.

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

Command+9's example uses this mesh viewport: 42 paths (1,056 segments), with a
blue reference cube when stock is disabled, mesh depth 7, and a 500,000-fuel budget
including Grap ball-radius compensation. Its [memo graph](incremental.md) records
the path, prepares stock/tool geometry, and meshes only when observed inputs
invalidate those stages. Camera changes render a fresh image from retained geometry.
Native builds use GPU triangle drawing with synchronous readback;
web/headless fallback uses the same geometry in the CPU triangle renderer.
Expand `panes` to change `mesh depth` or replace `preview paths mesh` with
`preview paths 3d` for comparison. Drag to orbit and scroll to zoom. The document
contains its own copy of the cube definition so it is self-contained. Its
cube parameters drive both the reference solid and the toolpath's contact points
and normals. The initial stock bounds remain independent: making a smaller part
does not silently shrink the block being machined.

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
the current mesh viewport, which bypasses the GPU VM entirely. It now retains
geometry through the general dependency graph.

## Playback

The mesh preview accepts an optional `playback` record with `progress` (f64,
0–1), `ball radius`, `tool length`, and `stock minimum` / `stock maximum` (f64
`x`, `y`, `z` records). These are ordinary data, not control state. The example
supplies progress from the reusable [controls](controls.md) library.

The recording computes total cutting-segment length, then visits segments again,
splitting the current segment at the requested distance. Completed segments are
hidden, upcoming segments use the requested path color, and the tool is orange.
The current segment is split exactly at the playback position: only its upcoming
portion is drawn. Path starts do not contribute
distance: the cursor jumps between disconnected passes rather than inventing
linking moves. Progress is not machining time. Empty paths have no tool; zero
length segments are well-defined. The visual tool is a vertical ball-end cutter,
with a hemispherical tip and a flat-topped cylindrical flute. Paths identify its
ball center, and length measures tip to top.
Playback is generic over the consumer's error type, with conversion from path
validation errors. The preview translates invalid geometry to ordinary absents.

Without `stock`, the stock bounds draw a wire envelope. With that field, its
record supplies an opaque `color`. The preview's ordinary `mesh depth` determines
the stock mesh resolution too. Command+9 starts with a one-inch cube, bounded
by −0.5…0.5 on all three axes, so the passes carve the indent into its top face
without first removing an oversized stock allowance.
Stock replaces the reference solid while enabled, avoiding coplanar surfaces
where their boundaries coincide. It shares the paths' depth buffer, so intact
material hides any path beneath it. There is no x-ray overlay or depth offset.
Slider changes rebuild stock and mesh while retaining the unchanged path recording.
Path color and line thickness affect only the path/tool geometry layer; the stock
expression and mesh are reused.
Orbiting retains geometry; every new view still produces a fresh raster image.

### Fidget stock removal

`toolpath::stock::Stock` starts with a box-shaped Fidget field. Each completed
segment, including the completed portion of the current segment, subtracts the
continuous swept solid of a vertical `BallEnd` tool: `stock.max(-sweep)`.
Fidget's ordinary CPU mesher builds triangles from the resulting expression;
the preview uses the same triangle renderer as other mesh views. There is no
heightfield, sampled stock grid, or fallback stock algorithm. The target model
does not participate in removal: a bad path can cut past the intended surface.
Stock mode does not mesh or draw that reference solid.

A sweep unions the moving ball with the moving finite cylinder above its
equator. The ball uses squared distance to a segment. For the cylinder, each
query Z restricts which portion of the motion can contain that point; radial
distance is minimized over that interval, with the overall end planes bounding
it vertically. These are closed-form Fidget expressions, not sampled tool
placements or a loop executed for each queried point. Horizontal, vertical,
sloping, and zero-length segments share the same solid semantics. The tool is
still fixed along +Z; orientation is not inferred from the path's tangent.

The remaining stock is a full 3D solid, so through cuts and material over a
cavity are representable. Meshing still approximates the implicit surface, and
coarse depths can visibly distort narrow grooves. Seeking backward reconstructs
the expression from the initial block. The memo graph retains the latest result,
not simulation history or a collection of meshes for earlier slider positions.
On native builds, expression construction and stock meshing run in the general
graph's background executor. Playback updates tool/path triangles immediately;
old stock is desaturated until its replacement is ready. The first pending frame
shows available tool/path geometry and an ellipsis. Cancellation uses Fidget's
octree token, and superseded results are discarded. See the
[async boundary and limitations](incremental.md#background-computations).
Disconnected starts still have no linking cut. No holder or collision model is
implied, and this models the programmed polyline, not controller-specific motion
blending or physical cutting behavior.

The example demonstrates top-face finishing only. At completion, stock remains
around that face and on the other five sides; it is not yet a roughing program
or a manufacturing simulation for the entire cube.

Variable orientation, general tool profiles (including arcs and separate cutting
and collision parts), explicit links, and machine/postprocessor output remain
separate next steps. Mesh/non-mesh mode controls and render-quality controls can
use the controls library later; path-generation tolerance belongs to the program
and is distinct from mesh/raster resolution. Upcoming-path windows, transparency,
and coarse-to-fine rendering are also deferred. In
particular, the existing Fidget cube field is not an exact signed distance;
subtracting a cutter radius from it would not implement a geometric offset.
