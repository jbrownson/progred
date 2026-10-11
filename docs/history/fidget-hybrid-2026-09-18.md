# Hybrid CAM rendering — 2026-09-18

CAM implicit previews now send only the model or remaining stock to Fidget.
The displayed tool and path tubes use the existing triangle generator. Cutting
sweeps still form the implicit stock subtraction; this changes visualization,
not the simulated cut.

Completed tiles carry RGBA and orthographic normalized depth together. A GPU
pass replaces completed draft-surface pixels, including empty rays, then draws
paths and the cutter against that depth. Uncomputed pixels retain the draft.
The CPU fallback follows the same ordering. Fidget samples are aligned to mesh
pixel centers. No Fidget dependency changes were required.

## Measurements

Headless Apple Silicon run of the current Command+9 program (6,588 segments),
333×750 pixels, native XY and 4× depth sampling. Three renders per variant,
median wall time; GPU triangle initialization warmed separately. These include
implicit preparation/rendering and hybrid GPU composition/readback, not editor
projection or final UI presentation. Recording is shared rather than rerun for
each variant.

| Playback | All implicit | Hybrid | Interpretation |
| --- | ---: | ---: | --- |
| 2% | 665 ms | 173 ms | About 3.9× faster |
| 50% | 3,866 ms | 3,676 ms | Little change; stock subtraction dominates |

At 2%, path/tool mesh generation took 6.6 ms and produced 921,600 triangles.
Hybrid composition took 9–21 ms; at 50%, 8–10 ms. These are experimental timings,
not frame-rate guarantees. The existing path meshes are quite dense; this pass
does not change their tessellation.

A follow-up uses analytic smooth normals for the tool and path tubes, and raises
only the displayed cutter from 12 to 64 circumference sides. At 2% playback this
adds 624 triangles (922,224 total). The 474,168 vertices carry 1,896,672 bytes of
normal attributes, using four-byte signed-normalized storage rather than three
floats. Path/tool generation measured 6.6 ms, versus 6.5 ms immediately before
this change. With the same geometry and vertex format, twelve alternating flat
and smooth draws measured median upload/render/readback times of 10.4 and 10.1 ms,
respectively: no meaningful shading regression in this check. Both variants carry
the normal attribute, so this does not isolate its bandwidth cost against the old
24-byte format. Add `CAM_MESH_SHADING=1` to the command below to run that comparison.

Reproduce with the ignored `cam_render_profile` test, setting `CAM_HYBRID=1`,
`CAM_PROGRESS=0.02` (or `0.5`) and `CAM_HEIGHT=750`. Use the repository's sandboxed
Cargo wrapper for compilation. Actual GPU comparison requires a headless process
with GPU access; the test never opens the app.

Model-mode tool movement, path colors and line width now retain the implicit
model image as well as the model mesh. Stock-mode playback still recomputes the
surface. Current mesh geometry remains available immediately on camera changes.

## Costs and checks

An orbit investigation after the smooth-normal change found no geometry
invalidation in the existing camera-reuse regression test: camera changes retain
the same mesh allocation and do not rerun the recorded program. Separate warm
headless canaries measured 9.9 ms median for the mesh viewport including controls
(800×1200 pixels), 9.1 ms for the source pane (1200×1800 pixels), and 0.32 ms for
the controls/declaration pipeline with rendering omitted.

A temporary full-editor timing probe used the actual refined projection, a
completed stock mesh, equal pane widths, scale 2, five warmups and 60 measured
camera changes. Implicit work was queued but not executed, isolating the main
thread from background contention. Times include frame preparation and drawing
into a draw list, including mesh GPU upload/render/readback; they exclude final
UI GPU composition and window presentation:

| Window, physical pixels | Playback | Median | p95 |
| --- | ---: | ---: | ---: |
| 1200×900 | 2% | 18.9 ms | 20.2 ms |
| 1200×900 | 50% | 11.8 ms | 12.4 ms |
| 2400×1800 | 2% | 19.4 ms | 21.3 ms |
| 2400×1800 | 50% | 12.7 ms | 12.9 ms |

The temporary probe was removed. These are current-cost measurements, not an
old/new regression comparison. The live-process sample mostly caught idle time
and cannot establish the cause of interactive hitches. Code inspection confirms
that mesh buffers retain capacity but repack and upload their contents every
draw, even for unchanged geometry. The GPU result also returns through CPU pixels
before upload to the UI compositor. Both costs predate the normal attributes;
retaining uploaded geometry and later handing off GPU textures are separate
optimization candidates, not dependency-invalidation fixes.

### Retaining GPU mesh uploads

The follow-up now freezes constructed geometry into a shared mesh. The renderer
retains the last uploaded mesh's index prefix and a weak allocation identity.
Camera changes reuse the buffers; geometry edits or a larger required prefix
upload again. It holds no strong reference to obsolete CPU geometry and changes
no memo dependency rules. Pixel rendering/readback still happens every draw.

Twelve alternating same-geometry GPU draws per variant, at 333×750 pixels, compare
the prior repack/upload behavior with retention. Detaching the weak identity
outside the timer forces the reupload case without changing any geometry. Both
paths assert byte-identical output:

| Playback | Reupload median | Retained median |
| --- | ---: | ---: |
| 2% | 9.70 ms | 1.65 ms |
| 50% | 3.41 ms | 1.53 ms |

Use `CAM_HYBRID=1 CAM_MESH_UPLOADS=1` with the existing `cam_render_profile`
test to reproduce. The mesh viewport orbit canary (800×1200 pixels including
controls) fell from 9.88 ms to 2.14 ms median, with p95 4.67 ms after retention.
Repeating the temporary full-editor 2400×1800 probe twice measured medians
11.26–12.45 ms at 2%, versus 19.43 ms before, and 11.47–12.70 ms at 50%, versus
12.69 ms before. The halfway full-editor gain is small/noisy despite the isolated
upload improvement. Source projection and final presentation remain separate
costs; this is not a claim about measured on-screen frame rate. One full-editor
run also had a 62.8 ms outlier, absent in its repeat. Temporary instrumentation
was removed; the opt-in paired upload benchmark remains.

Each retained raster now also owns four bytes of depth per pixel. Color and depth
share the publication lifetime; a renderer retains one uploaded pair and does not
upload it again for an unchanged image. Complete implicit frames skip uploading
the replaced draft mesh. Triangle output still uses the existing synchronous
readback; direct GPU texture handoff is separate work.

Tests cover analytic sphere depth under different camera angles/aspect ratios,
pixel-center alignment, front/behind mesh occlusion, replacing overly-near draft
geometry, empty versus unfinished pixels, resolution changes, and CPU/GPU parity
of those cases. Async regression tests cover current-mesh readiness, cancellation,
immediate tool movement, and model-image reuse. Rendering remains a visual
approximation, not a manufacturing/collision guarantee.

### Direct mesh composition

The follow-up removes the preview GPU → CPU → GPU image round trip. Puri's
`draw_mesh` carries shared geometry, a camera, and optional surface color/depth.
`SplitCanvas` preserves it as a mesh layer; the compositor renders and composites
it on its own device before reusing its scratch target for another preview.
No separate mesh device, blocking map, row copy, unpremultiplication, or final
RGBA upload remains in the production mesh path. CPU implicit results still
upload their color/depth when published. Program/mesh invalidation is unchanged.
CPU rendering remains an explicit interpretation for SVG/browser consumers.

Apple M3 Pro / Metal, release, 2400×1800 physical editor output at scale 2,
equal panes, completed stock mesh, camera changing on each frame. Twenty
measured pairs after four warmups alternate route order. Both routes retain
mesh uploads and raster targets. The round-trip baseline also retains its
readback buffer and uses bulk row copies. Both include full-editor vector/image
composition and GPU completion; neither includes window presentation.

| Playback | Round trip median | Direct median | Frame construction/recording (separate) |
| --- | ---: | ---: | ---: |
| 2% | 6.56 ms | 3.98 ms | 10.16 ms |
| 50% | 6.51 ms | 3.87 ms | 10.32 ms |

The baseline uses the same device and keeps pixels premultiplied, avoiding the
old extra alpha-conversion loop: this isolates the transfer rather than exactly
reconstructing every historical cost. Final full-editor pixels are byte-identical.
The direct route makes zero compositor image uploads. The test does not measure
background Fidget contention. The slowest 2% direct draw was 11.54 ms;
the slowest 50% direct draw was 5.30 ms. Desktop timing is noisy, so this supports a typical
~2.6 ms saving, not an across-the-board latency or on-screen frame-rate guarantee.

Reproduce with the ignored
`projection::tests::frame::compositor::mesh::cam_mesh_roundtrip_profile` test.
The GPU tests separately check same-sized target reuse across multiple previews,
rectangular/rounded clips, vector controls above meshes, geometry edits,
depth-only publications, and premultiplied alpha. The CPU interpretation remains
covered by camera, depth, shading, and full-editor SVG tests.

The large-scene hover regression exposed a separate repeated-render shading quirk:
rendering the same 2118×1836 mesh repeatedly changed about 1,700 color channels
between its first and second draws (maximum difference 13/255), then became
byte-stable in that standalone probe. A single warmup did not consistently remove
the smaller variations when interleaving full-editor draws. The standalone probe
reproduced this on both the compositor's device
and the prior Fidget `Gpu::init_basic` device configuration, without any hover or
Vello composition. The cause is not established. Fine-derivative hints do not
change the generated Metal shader, so no shader workaround was retained. The
hover test now verifies unchanged mesh inputs separately from exact restoration
of the bottom controls' pixels. Its hovered-image check still rejects large
pane changes such as disappearing controls. The small deterministic mesh tests
retain their whole-image comparisons. Production does not issue a warm-up draw.
The shading variation is deferred, not silently repaired or attributed to hover.
