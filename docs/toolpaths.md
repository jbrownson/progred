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
`ball-center passes` wraps the contact generator with that mapping. The editable
`tool diameter` is 0.125 inches; both this mapping and the preview convert it to
a radius of 0.0625. The first fixture covers one
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

`preview paths refined` takes the mesh preview's arguments and composes a mesh
fallback with progressive software implicit images. It shares one observed path
evaluation between the two interpretations. Both computations are requested,
but neither needs the other's result. A current implicit image replaces the
mesh; pending implicit work displays the available mesh at the current camera.
An outdated stock mesh is desaturated, while tool/path geometry updates immediately.
Implicit refinement starts at no more than 512 physical pixels on the longest
edge, skipping the standalone implicit renderer's two coarsest levels. It roughly
doubles XY resolution up to native size, then finishes with four-times depth
sampling. The mesh supplies immediate feedback until the first current implicit
image. Standalone mesh and implicit functions remain available.

Command+9's example uses this refined preview with one playback slider: 42 paths (1,056
segments), with a blue reference cube when stock is disabled and a 500,000-fuel
budget including Grap ball-radius compensation. Its [memo graph](incremental.md)
retains the shared path recording and both renderers' expensive results.
Implicit requests a new software image in the background when inputs change,
including the camera. Mesh retains geometry across camera changes; its optional
`mesh depth` defaults to 6 (the previous mesh-only fixture used 7).
It uses GPU triangle
drawing on native builds, with a CPU triangle renderer for web/headless fallback.
The example's Grap view calls `preview paths refined`; it contains no render-mode
state or radio buttons. Orbiting immediately returns to the retained mesh, then
matching implicit images take over when ready. Geometry changes request both a
new stock mesh and new images. There is no special handling of drag events or
inactivity delay. Both interpretations share camera framing and camera-space
lighting (including conversion from Fidget's downward-pointing sample Y axis).
Drag to orbit; scroll or pinch the Mac trackpad
over the viewport to zoom. Pinch needs no modifier and shares the existing
per-view camera state in both mesh and implicit previews. The document
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

All three volume previews accept an optional `playback` record with `progress` (f64,
0–1), `tool diameter`, `tool length`, and `stock minimum` / `stock maximum` (f64
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
ball center, and length measures tip to top. Diameter is converted to radius
at the playback boundary. Both must be finite and positive, and tool length
must be at least its diameter for this ball-end model.
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
Slider changes retain the unchanged path recording. In the mesh preview,
path color and line thickness affect only the path/tool geometry layer; the stock
expression and mesh are reused.
Orbiting retains mesh geometry; every new view still produces a fresh raster image.
The implicit preview renders stock, tool and paths together in one progressive
async request. Its tool moves with each current image, not independently of the
stock. The old image is dimmed until the first current coarse image arrives;
current refinements restore normal colors, with an ellipsis until the final
depth pass completes.
The first pending frame reserves the viewport and shows an ellipsis. Refinement
starts at at most 128 physical pixels on the longest edge, then doubles toward
native resolution with a fixed camera and render volume, then performs one
native-size pass with four times the depth samples to reduce sharp-edge artifacts.
The normal native-resolution image remains visible during that last pass.
New input cancels the old sequence. The controls overlay the bottom of the
full-size view.
Implicit CAM rendering explicitly uses the software voxel renderer, directly
on the existing background executor. It does not attempt GPU evaluation first.
Software rasterization uses 32/16/8-pixel tiles to bound uninterrupted work more
finely. Lighting corrects sample-space gradients for unequal axis spacing, so
depth refinement does not change the light direction or relative axis weighting.
Web/default headless contexts use inline execution.

**Experimental limitation (2026-09-14):** the CPU implicit captures complete and
show correct playback. A native Metal run of the stock-removal example waited
over a minute in GPU readback and was terminated. Small GPU color/constant-field
tests pass. The bytecode diagnostic after accepting the updated Xcode license
confirmed unsupported memory instructions in the stock expression: at progress
0 it has 15 instructions and no memory operations; at 0.35 it has 17,473
instructions including 794 loads/stores; at 1.0 it has 53,228 instructions
including 12,914 loads/stores. The pinned GPU interpreter and tape simplifier
both leave `OP_MEM` unimplemented. Async scheduling cannot make that bytecode
valid on this backend. A further native submission of this known-unsupported
program was deliberately avoided. Command+9's implicit refinement uses the explicit
software path; GPU implicit CAM needs an upstream implementation before use.
No GPU timeout or spill emulation was added. Ordinary Fidget voxel previews
retain their existing backend selection. The ignored
`editor_toolpath_implicit_async_svg_captures` test captures the checked-in
example without opening a window; standalone-renderer captures substitute the
preview function in the test fixture.
`editor_toolpath_progressive_svg_captures` captures intermediate resolutions
through the full editor's normal async polling and frame pipeline.
`editor_toolpath_refined_svg_captures` captures mesh/image handoffs, orbit fallback,
and stale stock during playback with the production refined declaration.

### Fidget stock removal

`toolpath::stock::Stock` starts with a box-shaped Fidget field. Each completed
segment, including the completed portion of the current segment, subtracts the
continuous swept solid of a vertical `BallEnd` tool: `stock.max(-sweep)`.
The mesh preview uses Fidget's CPU mesher and the shared triangle renderer.
The implicit preview renders the same expression directly with the voxel renderer.
There is no
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
In the native mesh preview, expression construction and stock meshing run in the
general graph's background executor. Playback updates tool/path triangles immediately;
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
separate next steps. Render-quality controls can use the controls library later;
path-generation tolerance belongs to the program
and is distinct from mesh/raster resolution. Upcoming-path windows and transparency
are also deferred. In
particular, the existing Fidget cube field is not an exact signed distance;
subtracting a cutter radius from it would not implement a geometric offset.
