# Toolpaths

The toolpath library is an experimental path-geometry layer, not a machine
program or a collision/clearance check. Its geometry remains library-owned;
the mesh preview uses the general dependency-tracked computation runtime.

## Generation and interpretation

Rust generators call `paths::Sink::start_at(point, axis)` and `line_to(point)`
with finite 3D tool-tip coordinates. The normalized axis points from the tip
toward the spindle and stays fixed until the next start (3+2 positioning, not
simultaneous orientation interpolation). A new start begins a separate path: it is neither a cutting link
nor a rapid move from the previous endpoint. A line requires a preceding start.
Calling generators in sequence composes their output. `MapPoints` adapts another
sink, so translation, reflection, surface mapping, and further adapters compose
without recording intermediate paths. It maps positions only, preserving axes;
an arbitrary surface mapping does not define a cutter orientation.

`Recording` is an optional initial representation: native start/line commands
and nested tool groups. Playback uses it for total distance and seeking; tests also
replay it into other sinks. The line preview instead consumes
emitted points directly into one projected vector path and its bounds; the 3D
voxel preview consumes them into native Fidget fields. The mesh preview retains
an observed recording, then emits tube vertices and indices from it. There is
no GID command list or opaque Rust program passed through Grap. Final-encoding
generators can still run directly against any sink without recording.

Grap's `start at` accepts an optional `tool axis` x/y/z record, defaulting
to +Z. Explicit zero or nonfinite axes fail without emitting a command.
Grap uses ordinary calls to scoped `start at`, `line to`, `with tool`,
`map points`, and `map axes` functions. `do` supplies sequencing; lambdas supply reusable
generators. Emitters outside an installed output scope return `toolpath output
required`. `map points` takes a callable under `mapping` and an unevaluated
`expression`. For each emitted point, it calls the mapping with `x`, `y`, `z`;
the mapping returns an ordinary record with those fields (the `point` function
constructs one). Nested mappings run inside-out and restore the surrounding
mapping on exit, including on failure. Mapping functions are intended to
compute coordinates, not emit more paths.

`map axes` has the same scoped mapping/body interface, but maps the spindle-facing
direction on `start at`, leaving positions untouched. The resulting axis must be
finite and nonzero and is normalized before emission. Nested axis maps apply
inside-out; both kinds restore their scope on success, absence, and evaluator
halt. Maps should be pure. Mapping a path's axis does not change the axis of an
already-started path: a new orientation requires a new `start at`, as before.
The example's ordinary Grap `rotate paths(orientation, program)` composes both
maps with the same rotation callable. Its input must be a linear rotation about
the origin, not a point mapping with translation or arbitrary deformation.
Translate the resulting path separately with `map points`; translation must not
affect a direction vector. Neither operation records intermediate paths.

`with tool` takes a `tool` expression and an unevaluated `expression` body.
It evaluates and validates the tool once, then runs the body in that tool's
scope. Nested scopes override locally; leaving restores the enclosing tool,
including after a returned absent or evaluator halt. The body's result is
returned unchanged. Entering and leaving a scope end the current path: the next
line needs a new `start at`. There is no inferred rapid, linking cut, or physical
tool-change motion. Unscoped paths remain drawable, but stock simulation rejects
segments without a tool, including upcoming ones.

Native code uses `paths::with_tool(sink, tool, body)`. The sink's balanced
enter/leave notifications let streaming consumers interpret scopes without
recording; point-only consumers just break the path. The recording interpreter
retains nested groups and replay preserves them. These notifications are not
separate Grap commands or a persistent global current-tool setting.

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
It returns an ordinary geometry record: `field` constructs the implicit solid,
`top face` maps UV points to that surface, `normal` gives the outward surface
normal at a contact point, and `chamfer strip` describes a canonical planar
chamfer. The strip contains a centerline `curve(t)`, its `length`, its `width`
(`c√2`), the outward `normal`, and the unit `across` direction. These outputs
share the underlying dimensions. Paths and implicit geometry are constructed
from that common description; straightforward paths are not recovered from the
implicit field or mesh.
The implicit field is constructed only when requested; path
generation does not construct or mesh a solid it does not consume. Dimensions
and path arithmetic use f64; `f32 from f64` explicitly rounds the constants at
the Fidget construction boundary.

`crosshatch` takes the top-face mapping, rigid orientation, setup-up, and a
compensation function from the current fixed axis to a point mapping.
`face passes` obtains the surface and normal functions from the cube, then
composes surface mapping, ball-radius compensation, orientation, and the
ball-center-to-tip shift.
Neither CAM function contains cube dimensions or a duplicate
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
finishing program, this example leaves the passes disconnected.
The crossing family reverses its row order, as Rhino does. The separate Grap `ball center` mapping
offsets each contact point along the analytic surface normal by `ball radius`;
`ball tip` then applies the orientation and subtracts radius times the tool
axis. The editable `tool diameter` is 0.125 inches; this mapping and the
ball-tool constructor share that cell. `Op 1 · top and four sides` calls that same generator for
+Z, +X, +Y, −X, and −Y. The separately callable `Op 2 · bottom` uses
the same generator for −Z, rotating the top-face coordinates 180° about X.
The six signed-axis rotations and all machining policy are ordinary Grap
functions in the document. `ball-tip passes` remains the top-only generator.
`Preview · Op 1 + Op 2` sequences the two programs for the playback slider,
retaining Op 1's removed stock into Op 2. It is a preview composition, not a
single machine program: eventual export should target Op 1 and Op 2 separately.
Both currently use part coordinates; there is no simulated stock flip,
work-offset definition, or connecting move between setups.
Each operation sequences a `with tool: ball tool` indent group and a sibling
`with tool: square tool` chamfer group; playback supplies no global cutter.
Op 1 cuts the four top and four vertical chamfers. Op 2 cuts the four
bottom edges. The square tool switches in at those group boundaries without
inventing a tool-change motion.

### Chamfer composition

The example keeps algorithms as ordinary Grap functions, not a strategy enum
or a record containing the union of every strategy's settings:

- `straight cut(start, end, tool axis)` emits one disconnected straight path.
- `evenly spaced(length, stepover, action)` calls `action(t)` at normalized
  positions including both endpoints. Length and maximum stepover must be finite
  and positive; intervals are `max(1, ceil(length/stepover))`. It streams calls,
  not a list. A consumer using a curve must supply its arc length and an
  arc-length-normalized parameterization; the current strip has a straight curve.
- `parallel strokes(curve, length, offset, stepover, stroke)` uses that sampler,
  calling `stroke(start, end)` from `curve(t) - offset` to `curve(t) + offset`.
  It knows nothing about the cutter or its axis. `stroke` can be a straight cut
  or a different supplied computation.
- `square side contact offset(diameter, contact height, normal, tool axis)`
  computes the contact-to-tip displacement for a cylindrical side contact.
  The unit normal and unit axis must be perpendicular. This is specific contact
  geometry, not a general compensation solver for arbitrary profiles.
- `contour chamfer(diameter, contact height)` and `crosswise chamfer(stepover)`
  return ordinary closures accepting a `strip`. Configuration needs no output
  scope; calling the configured closure emits paths. Neither recipe knows the
  cube's angle, edge rotations, or global parameter cells.
- `four rotated copies(program)` (binder `chamfer_ring`) executes a supplied
  zero-argument program at four quarter-turns. It has no cutter/spacing/geometry
  parameters. `cube chamfers` supplies the configured program: first orient the
  canonical strip's cut, then repeat that oriented cut around world Z.

`end-cut chamfer pass` (`chamfer_pass`) and `side-cut chamfer pass`
(`contour_pass`) are the two configured recipes exposed in the source list.
The `cut strip` binding in `cube chamfers` selects one, before repetition.
Change that binding's subject from the end-cut pass to the side-cut pass to
switch recipes. The contour configuration shares diameter and cutting-length
cells with the square profile and explicitly chooses half the cutting length
as contact height. The end-cut configuration only reads stepover. Tool selection
remains the enclosing `with tool` group, separate from these contact functions;
choosing a different tool shape still requires a matching contact recipe.
The recipes do not infer tool compatibility from an arbitrary profile.

### Side-contour chamfers

The cube's chamfer strip `curve(t)` returns `(s/2 − s*t, (s−c)/2, (s−c)/2)`:
the top/front chamfer centerline, extended to the stock's ends. `contour chamfer`
puts the side of the square mill tangent to this plane, at the configured
contact height. With outward normal `n = (0,1,1)/√2`, spindle-facing axis
`a = (0,−1,1)/√2`, cutter radius `r`, and cutting length `L`, the tip is
`contact + r*n − (L/2)*a`. It emits one straight start/line pair. Feed is
`a × n`, following the example's clockwise climb convention. Repetition and
rotation are composed outside the recipe, as described above.
There is no Rust chamfer generator or mesh-derived path.

The square profile is a Grap quote with spliced `square tool diameter` and
`square cutting length` cells. The contour compensation reads those same
cells; the tapered non-cutting neck and wider shank remain explicit profile
data. Its defaults are 0.125-inch diameter and 0.22-inch cutting length.
The canonical pass has sufficient diameter and cutting length for the default
0.1-inch chamfer. This is a single full-length finishing pass, not a roughing
strategy or an automatic check that arbitrary dimensions cover the chamfer.
Changing the profile to a different cutting shape would also require changing
this square-side compensation. The vertical-edge tool axes are horizontal;
there is no fixture/holder clearance claim.

Tests compare the actual Fidget swept subtraction for all twelve edges with
the beveled cube's planes, and check edge coverage, tool scopes, compensation,
and updates from edited cube/tool dimensions.

### Crosswise end-cut chamfers

The example defaults to the alternative `crosswise chamfer` callable. The square
mill's axis is the strip's outward normal `n`, with its flat tip directly on the
plane. At each centerline sample, `parallel strokes` emits one straight cut from
`center − (width/2)*a` to `center + (width/2)*a`, where
`a = (0,−1,1)/√2` is the across-chamfer direction. Both endpoints and the axis
receive the ring's rigid orientation. No ball-radius or side-radius compensation
is appropriate here: the tip plane itself is the contact plane.

`chamfer stepover` defaults to 0.05 inches. For a straight edge of length `s`,
the program uses `max(1, ceil(s/stepover))` intervals and includes both endpoints.
The actual spacing is therefore no greater than the requested maximum. Every
row cuts in the same direction; progression along the edge is
`feed × normal`, using the same row-order convention as the indent passes.
Each row starts a disconnected path: no return stroke, rapid, or linking cut
is inferred. The row loop is Grap `iterate`; emission uses the existing
`start at`, `line to`, and `map points` capabilities.

Stepover must be finite and positive, and the computed interval count finite.
Finer requests remain bounded by evaluator fuel, not silently capped. Stepover
is not clamped to the tool diameter: oversized steps leave real uncut strips in
the subtraction. Both recipes assume a straight planar strip with constant,
orthonormal normal/across directions and a square cutting profile. The cube
supplies a 45-degree strip, but that angle is not part of the recipes. They simulate the ideal revolved envelope,
not tooth marks, runout, or surface-finish physics. Tests cover both swept solids,
all edge orientations, width/spacing changes, invalid spacing, visible gaps from
wide steps, and dependency invalidation when the callable or spacing changes.
Independent-strip tests also exercise both recipes away from the cube's 45-degree
frame; the shape and direction are explicit inputs rather than hidden constants.

### Indent tilt and direction

The editable `tilt (degrees)` defaults to 45 and accepts any finite angle;
there is no machining-policy range guard. Zero points along the face normal,
90 degrees along the chosen feed direction, and other angles use the same
trigonometric calculation. The example assumes the usual clockwise spindle
rotation, viewed from the spindle toward the tip. Each operation passes an
explicit `setup up` vector: +Z for Op 1, −Z for the flipped Op 2, expressed in
part coordinates.
The ordinary Grap `pull direction` function selects the sign of each diagonal
so its lean points toward setup-up. A horizontal tie keeps the positive
diagonal direction. The axis is
`cos(tilt) * face_normal + sin(tilt) * feed_direction`.
For positive tilts below 90 degrees, motion runs along that lean, pulling the
cutter, with row progression along
`feed_direction × face_normal` for the example's clockwise climb policy.
Choosing the opposite feed direction reverses both points within each pass and
the row sequence; the reflected crossing family also reverses its row sequence. All of this is
editable Grap, not a rule in the path sink or renderer.
Other angles need not preserve the upward-lean or pulling assumptions; accepting
the resulting geometry is not a collision-clearance or machining-safety check.

Tests check the spindle-facing hemisphere, pull direction, row progression,
and unchanged sampled geometry on all six faces. These are reference-face
rules, not a claim of collision clearance or verified engagement throughout
every curved cut. Tilt changes the tool and swept volume, not the ball-center
compensation. The example does not plan indexing/retracts, model a holder/fixture,
or perform collision checks.
The example interprets one model unit as one inch; the runtime still
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
evaluation between the two interpretations. Implicit work waits for the current
stock mesh; a geometry change cancels obsolete image work while meshing runs.
A current implicit image replaces the mesh; pending implicit work displays the
available mesh at the current camera. Orbiting after an implicit result therefore
returns to a current mesh, never an older stock result that it had overtaken.
An outdated stock mesh is desaturated, while tool/path geometry updates immediately.
Implicit refinement starts at no more than 512 physical pixels on the longest
edge, skipping the standalone implicit renderer's two coarsest levels. It roughly
doubles XY resolution up to native size, then finishes with four-times depth
sampling. The mesh supplies immediate feedback until the first current implicit
image. Standalone mesh and implicit functions remain available.

Command+9's example uses this refined preview with one playback slider: 504 paths (6,588
segments), with a blue reference cube when stock is disabled and a 3,000,000-fuel
budget including Grap ball-radius compensation. Its [memo graph](incremental.md)
retains the shared path recording and both renderers' expensive results.
Op 1 finishes its five indented faces and eight chamfers before Op 2 cuts the
bottom indent and four remaining chamfers. Slider intervals follow cutting
distance, including the chamfer passes. With the side-contour strategy selected,
there are instead 264 paths (6,348 segments). The operation boundary
adds no travel or cut.
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
0–1), `profile tolerance`, and `stock minimum` / `stock maximum` (f64
`x`, `y`, `z` records). These are ordinary data, not control state. The example
supplies progress from the reusable [controls](controls.md) library.

The recording computes total cutting-segment length, then visits segments again,
splitting the current segment at the requested distance. Completed segments are
hidden, upcoming segments use the requested path color, and the tool is orange.
The current segment is split exactly at the playback position: only its upcoming
portion is drawn. Path starts do not contribute
distance: the cursor jumps between disconnected passes rather than inventing
linking moves. Progress is not machining time. Empty paths have no tool; zero
length segments are well-defined. Paths locate the tool tip. The recorded group
supplies its validated `cutter::Tool` to mesh display, implicit display, and
stock subtraction; non-cutting sections are gray and do not remove material.
The cursor uses the tool of its current segment. At an exact segment endpoint it
stays with that segment until progress advances; scope boundaries add no distance.
Different tool groups subtract from the same stock in program order.
Tool geometry does not inherit dimensions from the path-line drawing style.
See [tool profiles](tool-profiles.md) for its line/arc representation, exact
linear-profile sweeps, and explicit arc approximation tolerance. The required
`profile tolerance` is part of playback settings, not the tool definition;
it controls curved-profile approximation for subtraction and tool display.
Playback is generic over the consumer's error type, with conversion from path
validation errors. The preview translates invalid geometry to ordinary absents.

Without `stock`, the stock bounds draw a wire envelope. With that field, its
record supplies an opaque `color`. The preview's ordinary `mesh depth` determines
the stock mesh resolution too. Command+9 starts with a one-inch cube, bounded
by −0.5…0.5 on all three axes, so the two operations carve six indents and twelve chamfers
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
current refinements restore normal colors. An unlabelled progress bar overlays
the top edge, resetting per refinement and disappearing when the final pass completes.
The first pending frame reserves the viewport and shows the empty progress track. Refinement
starts at at most 128 physical pixels on the longest edge, then doubles toward
native resolution with a fixed camera and render volume, then performs one
native-size pass with four times the depth samples to reduce sharp-edge artifacts.
The normal native-resolution image remains visible during that last pass.
New input cancels the old sequence. The controls overlay the bottom of the
full-size view.
Implicit CAM rendering explicitly uses the software voxel renderer, directly
on the existing background executor. It does not attempt GPU evaluation first.
On Apple Silicon macOS it uses Fidget's JIT with its recommended 64/16/8-pixel
tiles; other platforms retain the VM with 32/16/8-pixel tiles. Meshing stays on
the VM. The [local Fidget patches](../vendor/README.md) fix large AArch64 JIT
tapes and check cancellation within raster subtiles. Software scenes render all
objects within each image tile, retaining separate expressions and preserving
first-object wins on equal depths. Progress counts pixels in completed scene
tiles, excluding padding at image edges, rather than giving every object equal
weight. Its fixed total is the image's pixel count, not an estimate of time
remaining. Compilation and final image assembly/shading are outside that count.
Cancellation still cannot
interrupt a single tape compilation/evaluation already in progress.
Lighting corrects sample-space gradients for unequal axis spacing, so
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
continuous swept solid of the tool's cutting sections: `stock.max(-sweep)`.
The mesh preview uses Fidget's CPU mesher and the shared triangle renderer.
The implicit preview renders the same expression directly with the voxel renderer.
There is no
heightfield, sampled stock grid, or fallback stock algorithm. The target model
does not participate in removal: a bad path can cut past the intended surface.
Stock mode does not mesh or draw that reference solid.

The ball-profile lowering unions the moving ball with the moving finite cylinder above its
equator. The ball uses squared distance to a segment. For the cylinder, each
query's cutter-local Z restricts which portion of the motion can contain that point; radial
distance is minimized over that interval, with the overall end planes bounding
it vertically. These are closed-form Fidget expressions, not sampled tool
placements or a loop executed for each queried point. Horizontal, vertical,
sloping, and zero-length segments share the same solid semantics. The tool is
fixed along each path's explicit axis; it is not inferred from the tangent.
The same sweep is evaluated in an orthonormal cutter frame for tilted tools;
it is not approximated by discrete placements.

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

The example demonstrates indent and chamfer finishing in two operations.
Op 1 leaves the bottom indent and its four chamfers for Op 2. It is not yet
a roughing program or a manufacturing simulation for the entire cube.

Variable orientation, collision checking for the non-cutting profiles, explicit
links, and machine/postprocessor output remain
separate next steps. Render-quality controls can use the controls library later;
path-generation tolerance belongs to the program
and is distinct from mesh/raster resolution. Upcoming-path windows and transparency
are also deferred. In
particular, the existing Fidget cube field is not an exact signed distance;
subtracting a cutter radius from it would not implement a geometric offset.
