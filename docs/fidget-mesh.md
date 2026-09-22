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
(Command+9 / Ctrl+9) uses `preview paths refined`: a mesh draft followed by
final-quality implicit stock/model tiles. Paths and the displayed tool remain
directly generated triangle meshes, depth-tested against the implicit surface.
Standalone `preview paths mesh` and progressive `preview paths 3d` remain available.
The other Fidget examples retain
their voxel preview functions. Full-stock implicit GPU rendering is unsupported; see
[toolpaths](toolpaths.md#playback).

## Frame behavior

The ordinary Fidget mesh viewport lowers and meshes each scene object's field with
Fidget's CPU VM and Manifold Dual Contouring, in the supplied model-space
bounds. There is **no mesh or image cache**, including while orbiting, zooming,
resizing, or editing. The CAM `preview paths mesh` variant now composes a
[dependency-tracked recording/stock/mesh graph](incremental.md), retaining valid
geometry while camera views change. CAM stock construction/meshing runs
asynchronously on native and the threaded web build; ordinary Fidget previews
remain synchronous on a miss. CAM surface jobs finish and then catch up to the
latest request during dragging, without restarting on every move. The cutter
and remaining toolpaths keep following the playback slider immediately.
While CAM updates, the viewport retains desaturated old stock beside the current
tool/path.

The viewport emits Puri's backend-neutral `CanvasSink::draw_mesh`: shared
geometry, an orthographic camera, and optionally a raster surface with depth.
Native builds rasterize this on the compositor's existing GPU device through a
small WGPU pipeline with a depth buffer, opaque object colors, two-sided lighting,
and four-sample antialiasing. Its premultiplied texture is composed directly in
paint order, without a synchronous readback or re-upload. The compositor, not a
projection or pane, divides vector/mesh/image passes and preserves enclosing clips.
GPU pipelines, allocation capacity, and render targets are reusable resources. Completed geometry is a
shared `Arc<Geometry>` mesh; the renderer retains its uploaded vertex/index
buffers across camera changes. Upload reuse requires the same mesh allocation
and an already uploaded index prefix. A weak reference preserves that identity
without keeping obsolete CPU geometry alive; edits through `Arc::make_mut`
detach the identity. A different mesh replaces the retained upload. Ordinary
Fidget previews still construct a fresh mesh each frame; CAM retains geometry
through its computation graph. Camera inputs, depth, and pixels are overwritten
on every draw; this is not a rendered-image cache. Hybrid rendering also retains one uploaded surface
color/depth pair, keyed by image identity/dimensions and shared depth identity,
replacing it on publication. A depth-only edit also refreshes this upload.
Computation retention is outside it in the general computation
graph. Meshing and camera policy belong to the Fidget library; Puri's mesh
vocabulary and the native triangle interpreter know nothing about Fidget or CAM.

The default canvas interpretation, including web and SVG export, uses a
depth-buffered CPU triangle rasterizer without multisample antialiasing. This
requires no GPU initialization. The native compositor uses its required graphics
device and reports rendering errors through its ordinary error boundary. This is
not Fidget's voxel renderer as a fallback. Both triangle backends share the camera
transform and color/lighting policy. They reuse the ordinary Fidget viewport's
orbit and zoom handlers, image sizing, clipping, and per-view camera state.
Mesh and implicit views use the same height-based framing: changing pane width
reveals or crops space at the sides without changing the model's apparent scale.
Progressive implicit passes preserve that framing at every resolution.
Fidget's integer sampling grid is translated to pixel centers, matching triangle
rasterization. Published depth is normalized from Fidget's voxel depth to the same
near-zero/far-one interval as the mesh camera. Completed empty rays clear to far
depth; uncomputed pixels leave the draft intact. Depth is sampled without filtering.

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
Stock/model draft meshes use triangle normals. Path tubes interpolate analytic
capsule normals and retain their 12-sided circumference. The displayed cutter
uses 64 sides and vertex normals derived from its line/arc profile, with separate
normals at caps and profile corners. Normals use the GPU's four-byte
signed-normalized vertex format. These are viewport meshes, not a change to the
implicit model or CAM geometry.

`libraries::fidget::mesh` tests cover declaration validation, bounded depth,
constant fields, model/color changes, aspect ratio, zoom, and depth ordering.
The ignored `mesh_gpu_renders_and_reuses_buffers` test requires a real GPU and
does not fall back to CPU. It checks the shader, transparent background, empty
draws, non-aligned readback row widths, and resource resizing/reuse.

The existing `fidget_cube_profile_loop` exercises the mesh fixture and remeshes;
`fidget_toolpaths_profile_loop` follows the Command+9 fixture, recording deferred
mesh drawing. Its timing no longer includes GPU rendering; the paired
`cam_mesh_roundtrip_profile` covers drawing plus full-editor composition.
The updated default depth and cutter size differ from prior baselines.
The ignored
`editor_mesh_svg_capture` test captures the full editor with this example without
opening a window; `editor_toolpath_mesh_svg_capture` captures Command+9. These
use the real partial and the CPU interpretation of its mesh drawing operation.

See the [direct-composition measurements](fidget-hybrid-2026-09-18.md#direct-mesh-composition)
and the ignored GPU regressions for multiple same-sized previews, clipping,
vector overlays, depth-only changes, and transparent surface pixels.
