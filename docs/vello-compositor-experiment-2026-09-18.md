# Separate image composition experiment — 2026-09-18

The initial test-only experiment and its subsequent native integration, below.
Neither requires Vello or Fidget patches. This follows the
[wide-image atlas failure](graphics-memory-2026-09-16.md#wide-preview-flashing-follow-up-2026-09-18).

## Boundary

The experiment implements the existing Puri `CanvasSink`. Vector calls build
Vello scenes directly; image calls divide them into ordered vector/image layers.
Images upload into independent wgpu textures, never Vello's atlas. A separate
source-over pass composes each layer. This requires no changes to widgets,
projections, hover, or layout.

Clip scopes containing only vectors remain Vello clips. Scopes containing images
remain explicit groups: clipping their individual pieces separately would change
partially covered edge pixels. Pixel-aligned axis-aligned rectangles use GPU
scissoring. Other clips compose their children into a transparent intermediate
texture, then apply a mask rasterized by Vello. Transformed image boundaries also
use masks when they cannot use a scissor.

Vello's output uses straight alpha; the compositor converts it to premultiplied
alpha for blending. CPU images honor their declared alpha representation and
premultiply each sample before bilinear filtering. Intermediate compositor layers
are premultiplied. Final comparison scenes have an opaque background, like the
native editor; transparent final-output/export semantics are not established here.

There is also a backend-level GPU-texture layer. Its test source is created by a
GPU clear, not by the actual mesh renderer. It proves same-device texture handoff
without an intervening readback/upload, not migration of the production mesh
pipeline. That pipeline currently owns a separate GPU device and still returns
CPU pixels.

## Results

Apple M3 Pro, Metal, release build, Vello 0.9.0, headless execution. Two opt-in
tests passed; timing was repeated three times. No app was launched.

- Two 256 × 192 comparison scenes include overlapping translucent images,
  vector drawing before/between/after images, rectangular clips, nested circle
  and rounded-rectangle clips, and a rotated/scaled image. Maximum per-channel
  difference from direct Vello was 1 for the simpler scene and 2 for the nested
  scene, on the 0–255 scale. Both had 99th-percentile error 1.
- All eight large-image updates were visible through the compositor, including
  a changing image width. At 4200 × 2800, direct Vello missed two of eight updates.
  This resizes the image within a fixed output, as with a pane divider; it does
  not exercise live window/surface recreation.
- The same sequence composed GPU-only source textures correctly with zero image
  uploads in the composition stage. Readbacks occurred only after composition
  for assertions, outside timing.

Presentation-only median timings across the three runs:

| Output | Direct Vello, CPU images | Separate composition, CPU images | Separate composition, GPU-resident image |
| --- | --- | --- | --- |
| 1200 × 800 | 3.30–4.73 ms | 2.78–4.27 ms | 3.28–4.62 ms |
| 4000 × 2800 | 10.43–13.07 ms | 11.72–14.79 ms | 8.55–11.24 ms |

Each run takes six measured updates after two warmups. Timings include CPU
submission, uploads where applicable, and a wait for GPU completion. They exclude
scene construction, source-image generation, and diagnostic readback. Backends
run sequentially in a fixed order; clocks and other desktop activity were not
controlled. Treat these as a rough cost check, not a speedup claim. The 4200-wide
reference drops images, so its timing is not a useful performance comparison.

The preview-like case uses two Vello passes and three composition passes instead
of one Vello pass. Complex synthetic clipping uses more. The current prototype
also allocates full-output-size vector and mask scratch textures and one texture
per simultaneous nonrectangular clip depth. At 4000 × 2800, each RGBA8 scratch
texture is about 42.7 MiB. The mask scratch is currently allocated even when all
clips use scissoring. Resource lifetime and allocation policy need production
design; this is not a ready-to-ship memory policy.

## Scope identified before integration

The approach is viable for ordinary image draws, with a modest measured cost for
large CPU images. It can support GPU-resident mesh output later. Before adopting:

- Own textures on the application's device and retain unchanged uploads by image
  identity. Do not introduce widget identities or computation caches to manage
  graphics allocations.
- Bound/reuse scratch resources, especially for arbitrary clip groups; test window
  resize, multiple windows, transparency, and the real editor workload.
- Preserve the CPU image path for software rendering and exports. The prototype
  intercepts `draw_image`, not images used as arbitrary vector brushes; those
  would still go through Vello.
- Do not promise removal of the GPU mesh readback yet: partial implicit images
  currently combine with a CPU mesh fallback. Moving that combination to the GPU
  is another part of the production boundary design.

## Reproduce

The tests now exercise production code rather than retaining a duplicate
experimental implementation. Sources: [test entry](../progred/tests/compositor_experiment.rs),
[canvas interpreter](../ui/puri-vello/src/compositor/canvas.rs),
[GPU composition](../ui/puri-vello/src/compositor.rs), and
[blend shader](../ui/puri-vello/src/compositor/blend.wgsl).

Build with the repository wrapper:

```sh
./tools/sandbox-cargo test --release -p progred --test compositor_experiment --no-run
```

Run the resulting test executable with `--ignored --nocapture --test-threads=1`
in an environment with explicitly authorized GPU access. The Seatbelt build
environment does not provide that access. This is headless, not an app launch.
Pixel comparisons write reference, composite, and amplified-difference PNGs into
`target/compositor-experiment/` for inspection.

## Native integration follow-up

The native app now uses the same compositor exercised by these tests. The
browser and export interpreters are unchanged. Vector operations build Vello
scenes directly, split by draw-image operations and mixed clip scopes. There
is no per-pane pass count or widget-level layer API.

The device owns one compositor; each window owns separate `Resources`:

- Uploads are keyed by the immutable blob identity plus dimensions and format.
  The same bytes with different alpha interpretation share their upload; blending
  still honors each draw's alpha type. RGBA and BGRA are both supported.
- Images missing from the current paint release their textures before new uploads.
  There is no generation delay, retained history, or widget identity.
- One composition target supplements the surface's existing Vello target. Mask
  and clip-depth scratch textures are allocated only when needed, reused within
  and across frames, and dropped if no longer needed. Resizing recreates scratch
  textures, not otherwise unchanged uploads.
- An all-vector frame uses the original single Vello call, without a composition
  target. Leading vectors can render directly into an opaque composition target;
  this optimization does not cross clip scopes or reorder commands.
- The returned output explicitly identifies its alpha representation. Mixed
  output is premultiplied; direct Vello output is straight alpha. The native
  editor has an opaque base color, making both equivalent for its surface blit.

The headless regressions cover changing image and output sizes, independent
window resources, absent-image eviction, scratch release, shared uploads,
BGRA/straight/premultiplied inputs, transparent output, invalid image lengths,
nested clipping, and overlapping imagery. CPU images do not enter the atlas.
Image brushes still do. The synthetic GPU-source test is not a direct mesh
integration: mesh readback and partial implicit-over-mesh CPU assembly remain.

### Presentation measurements

Apple M3 Pro/Metal, release, 2400 × 1600 physical output. Three repetitions;
each has four warmup frames and twelve measured frames, alternating backend
order. No build ran alongside these repetitions, but desktop activity and GPU
clocking are uncontrolled. Numbers are ranges of the three run medians.

| Fixture | Direct Vello | Compositor |
| --- | --- | --- |
| Actual IoP editor frame, vector-only | 3.81–5.23 ms | 3.97–5.37 ms |
| Actual CAM editor frame, unchanged image | 1.72–1.95 ms | 3.51–3.88 ms |
| One synthetic preview, unchanged image | 1.53–2.98 ms | 3.23–4.52 ms |
| Two synthetic previews, unchanged images | 1.49–1.56 ms | 3.66–3.76 ms |
| Four synthetic previews, unchanged images | 1.56–1.59 ms | 4.60–5.81 ms |
| One synthetic preview, replaced image | 4.41–4.82 ms | 5.77–6.15 ms |
| Two synthetic previews, replaced images | 4.10–4.43 ms | 4.79–5.34 ms |
| Four synthetic previews, replaced images | 4.04–4.25 ms | 5.74–6.49 ms |

Actual editor timings include canvas submission/replay and GPU completion,
excluding projection, model evaluation, Fidget rendering, mesh rasterization,
and diagnostic readback. The comparison reuses the old Vello scene's allocation.
The captured CAM frame uses the mesh preview to avoid measuring a moving async
target. Synthetic timings exclude scene construction and image generation; the
total image area is constant as panes subdivide it. Warm unchanged frames
assert zero uploads; replacement frames assert exactly one upload per preview.

IoP stays at one Vello call and matches exactly. CAM uses two Vello calls and
three blends, with maximum pixel-channel difference 1 and p99 difference 0.
The synthetic one/two/four-preview cases use two/three/five Vello calls, so
their added cost is real, not eliminated by upload reuse. In the paired CAM
runs the extra presentation time is 1.79–1.95 ms. Four unchanged synthetic
previews add 3.04–4.22 ms. This is not an overall app speedup claim.

The wider replacement check also passes repeatedly: all eight 4200 × 2800
compositor images appear while direct Vello still omits two. That comparison
cannot establish relative speed, since the old route skips work.

Verification: 763 ordinary editor tests and four pure canvas grouping tests
pass; four headless GPU integration tests and the actual-editor comparison
pass in three repetitions. No application was launched. Run the real-frame
check by building the library tests and running their executable with
`editor_compositor_profile --ignored --nocapture --test-threads=1` under explicit
GPU access.

### Source-link hover follow-up

Command-hover over a generated chamfer leaf reproduced disappearing controls at
3028 × 1836 physical pixels, scale 2, with the preview occupying 70% of the
window. The control tree and hit targets remained present. Each matching notch
painted a tiny translucent rectangle inside a separate pane-sized clip; hundreds
of these clips caused the vector pass to leave its target unchanged. Direct
Vello rendering also failed, so this was not specific to the image compositor.

Vello 0.9 has fixed working-buffer capacities and skips painting after an
overflow; this is consistent with the observed failure, though allocator counts
were not read back. No backend patch or buffer-policy workaround was needed:
rectangular source feedback now fills the intersection of its rectangle and its
placement clip, without creating a clip layer. The full-size headless
`cam_hover_compositor_pixels` regression verifies that hover changes only a
small portion of the preview and that leaving restores it. An ordinary CPU test
also checks the clipped rectangle and absence of extra clip layers.

## Direct mesh follow-up

Mesh viewports now emit a backend-neutral Puri draw operation rather than a CPU
image. The compositor renders these in order on its existing device and blends
their textures directly. The earlier experiment's separate-device/readback
limitation no longer applies to the production mesh path. CPU implicit surface
publications still upload when they change. See the
[implementation and paired measurements](fidget-hybrid-2026-09-18.md#direct-mesh-composition).
