# Fidget mesh viewport

`preview mesh` is an alternative to `preview 3d`, not a change to Fidget's data
language or the editor's projection modes. It takes the same `value`, logical
`width`/`height`, and f32 minimum/maximum bounds on all three axes. `value` can
be one field or the existing opaque-colored scene convention.

An additional `mesh depth` argument is a u64 from 1 through 8, defaulting to 6.
This is maximum octree depth, not a surface-error tolerance. Each extra level
halves the finest cell spacing and can substantially increase work. The upper
bound keeps this synchronous experimental control from accepting unbounded
depth; it is not a general resource guarantee for arbitrary fields or scenes.
The function returns an ordinary `{preview mesh: {...}}` declaration, recognized
by a partial in the Fidget library. Invalid arguments return absents or decline
to the structural fallback, as with the existing preview.

Examples → Fidget cube (Command+8 / Ctrl+8) explicitly uses this function at
depth 5. Expand the document's `panes` field to edit the call, change its depth,
or replace its function with `preview 3d` for comparison. The toolpath example
(Command+9 / Ctrl+9) uses `preview paths mesh`: the same meshed cube plus directly
generated tube triangles in one depth buffer. The other Fidget examples retain
their voxel preview functions.

## Frame behavior

The ordinary Fidget mesh viewport lowers and meshes each scene object's field with
Fidget's CPU VM and Manifold Dual Contouring, in the supplied model-space
bounds. There is **no mesh or image cache**, including while orbiting, zooming,
resizing, or editing. The CAM `preview paths mesh` variant now composes a
[dependency-tracked recording/stock/mesh graph](incremental.md), retaining valid
geometry while camera views change. Native CAM stock construction/meshing now
runs asynchronously; ordinary Fidget previews remain synchronous on a miss.
While CAM updates, the viewport retains desaturated old stock beside the current
tool/path. Web remains synchronous until a browser-worker executor is added.

Native builds rasterize the resulting indexed triangles through a small WGPU
pipeline with a depth buffer, opaque object colors, flat two-sided lighting,
and four-sample antialiasing. The image is read back synchronously and submitted
through the existing canvas image operation. GPU device, pipeline, allocation
capacity, and render targets are reusable resources; geometry, camera inputs,
depth, and pixels are overwritten on every frame. There is no retained result
in this rendering backend. Retention is outside it in the general computation
graph. Rendering belongs to the Fidget library; there are no Fidget-specific
cases in Grap, Puri, or the layout algebra.

Web and GPU-unavailable native/headless environments use a depth-buffered CPU
triangle rasterizer, without multisample antialiasing. GPU initialization and
render failures report to stderr before switching to CPU. This is not
Fidget's voxel renderer as a fallback. Both triangle backends share the camera
transform and color/lighting policy. They reuse the ordinary Fidget viewport's
orbit and zoom handlers, image sizing, clipping, and per-view camera state.

Viewing bounds are also meshing bounds: geometry outside them is not extracted.
Zooming out cannot recover cropped geometry; expand the bounds instead. A mesh
may have open edges where the surface crosses those bounds. An everywhere
positive/negative/zero constant field produces no mesh surface here. Empty scenes
render transparently. Colors are not CSG: scene meshes are depth-tested together,
and earlier objects win equal-depth ties.

## Checks and limitations

The [initial experiment](fidget-meshing-2026-09-13.md) measures generation cost and
records mesh-quality caveats, including zero-area triangles. The viewport does
not silently weld or repair them and makes no manufacturing-validity guarantee.
Normals come from triangles, so coarse facets are visible. These are viewport
meshes, not a change to the implicit model or CAM geometry.

`libraries::fidget::mesh` tests cover declaration validation, bounded depth,
constant fields, model/color changes, aspect ratio, zoom, and depth ordering.
The ignored `mesh_gpu_renders_and_reuses_buffers` test requires a real GPU and
does not fall back to CPU. It checks the shader, transparent background, empty
draws, non-aligned readback row widths, and resource resizing/reuse.

The existing `fidget_cube_profile_loop` and `fidget_toolpaths_profile_loop` now
exercise mesh fixtures; the toolpath canary now reuses valid geometry, whereas
the cube canary remeshes. Their earlier voxel timings are not
unchanged baselines. The ignored
`editor_mesh_svg_capture` test captures the full editor with this example without
opening a window; `editor_toolpath_mesh_svg_capture` captures Command+9. These
use the real partial and normal backend selection.

Broader memo/async integration and direct GPU-texture composition remain future
work. In particular, this version still pays upload/readback costs
even though it uses GPU triangle drawing.
