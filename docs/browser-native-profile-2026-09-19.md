# Browser versus native CAM performance — 2026-09-19

## Scope

Headless measurements of the **actual Command+9 document**, not the small
spherical-cut worker smoke test. No editor or application window was launched.
The optional `cam-profile` feature and example compile the same diagnostic
against both native and shared-memory WebAssembly builds. They do not change
the normal editor's behavior.

Hardware: Apple M3 Pro, 5 performance + 6 efficiency cores, 36 GiB RAM;
macOS 27.0. Browser: headless Google Chrome 152.0.7977.76.
Release builds: native stable Rust; browser pinned nightly-2026-08-27, rebuilt
atomic/shared-memory std, as used by `make build-web`. This is a practical
comparison of current builds, not a controlled compiler-version experiment.

Both builds construct the document's `program_tree`, record its 6,588 segments,
and subtract the played portion from the same one-inch stock cube. The preview
is 512×512 **physical pixels**, yaw 30°, pitch 60°, bounds ±0.85; stock meshing
uses depth 6 and implicit rendering uses the application's final-quality 4×
depth sampling (2,048 samples deep here). Remaining paths and the tool are
meshes, not extra Fidget objects.

The diagnostic deliberately bypasses cross-run memo reuse. Program construction
and recording normally remain cached across orbit/playback changes. These are
fresh-work costs, not the cost of every editor frame.

## Baseline results (single worker, CPU mesh drawing)

Four runs per playback position; discard the first and report the median of
the remaining three. Drawing medians pool seven retained-geometry frames per
run after discarding each run's initial draw/upload.

Near the start (2% played):

| Work | Native | Browser | Browser/native |
| --- | ---: | ---: | ---: |
| Build program tree | 8.6 ms | 9.9 ms | 1.2× |
| Record full path program | 250 ms | 301 ms | 1.2× |
| Construct remaining-stock expression | 0.34 ms | 1.42 ms | 4.2× |
| Construct remaining path/tool mesh | 7.5 ms | 13.4 ms | 1.8× |
| Mesh stock | 105 ms | 452 ms | 4.3× |
| Final implicit stock image | 149 ms | 1,548 ms | 10.4× |
| Draw available mesh at a changed camera | 1.59 ms (GPU) | 52.8 ms (CPU) | 33× |

At the midpoint (50% played):

| Work | Native | Browser | Browser/native |
| --- | ---: | ---: | ---: |
| Build program tree | 8.6 ms | 9.7 ms | 1.1× |
| Record full path program | 251 ms | 299 ms | 1.2× |
| Construct remaining-stock expression | 5.8 ms | 5.2 ms | 0.9× |
| Construct remaining path/tool mesh | 2.3 ms | 4.9 ms | 2.1× |
| Mesh stock | 1.35 s | 9.03 s | 6.7× |
| Final implicit stock image | 2.19 s | 34.33 s | 15.7× |
| Draw available mesh at a changed camera | 1.58 ms (GPU) | 25.3 ms (CPU) | 16× |

The start-of-playback geometry has 925,468 triangles, mostly remaining toolpaths.
At the midpoint it has 309,346 triangles: fewer paths remain, while stock
meshing and implicit evaluation get more expensive as cuts accumulate.

For comparison, the *native CPU fallback* takes 39.2 ms near the start and
20.1 ms at the midpoint. Browser CPU mesh drawing is only 1.35× / 1.26× those
times. The large drawing gap is GPU versus CPU, not chiefly WebAssembly.
Browser CPU drawing alone already exceeds a 60 Hz frame's 16.7 ms budget;
this is not a measurement of end-to-end FPS.

The page's 10 ms timer had a maximum observed gap of 12.88 ms across the eight
worker trials. Midpoint implicit jobs published 147 partial snapshots each,
but this test discarded them rather than presenting them. Shared WASM linear
memory stabilized at 136,708,096 bytes in these runs (about 130 MiB); this is
not the browser's total process memory or a measurement of live allocations.

All native/browser repetitions agreed on segment count, path triangle count,
stock vertex/triangle counts, occupied pixels, and aggregate color/depth sums.
The two positions produced respectively 1,624 / 12,883 stock vertices and
3,244 / 25,762 stock triangles. Color sums were 24,947,516 / 25,043,699;
depth sums were 234,738.63623046875 / 235,529.66748046875.

### Interpretation

The async change prevents long Fidget work from occupying the browser event
loop; it does not make the computation faster. Ordinary Grap preparation is
fairly close to native here. Stock meshing uses the VM on both platforms but
gets native parallelism, while native implicit rendering also gets Fidget's
JIT. Mesh interaction has an independent CPU-versus-GPU presentation gap.

The measurements point to browser backend work rather than a need to redesign
the core computation/memo interface. A browser triangle renderer would address
the measured orbit drawing cost; a worker pool could recover some parallel
Fidget performance. Both are implemented and measured in the follow-up below.

## Backend control

One additional midpoint run renders through Fidget's scene API with backend
and thread count selected explicitly. These times exclude root preparation,
which cost approximately 141–171 ms, and exclude final RGBA shading, progress
publication, and presentation. They are not identical to the production-request
timings above. Native runs occurred under the Cargo sandbox, before the drawing
comparison was added.

| Backend | Threads | Implicit render |
| --- | ---: | ---: |
| Native interpreter | 1 | 28.71 s |
| Browser interpreter | 1 | 34.70 s |
| Native interpreter | 11 | 4.96 s |
| Native JIT | 1 | 7.96 s |
| Native JIT | 11 | 2.10 s |

In this control, WebAssembly's single-threaded interpreter is **1.21×** the
native interpreter time. The much larger application-level gap mostly comes
from the current browser backend lacking native's Fidget JIT and parallel
rendering. Browser WebAssembly is itself compiled by Chrome, but the compiled
code still interprets Fidget instructions; it does not get Fidget's native
specialized machine code. No extra browser Fidget worker pool was enabled.

All five control renders had 46,096 occupied pixels and raw depth sum
54,506,153. Matching these aggregates is a consistency check, not an exhaustive
pixel-for-pixel equivalence test.

## Boundaries

- Implicit times include the actual request's shape compilation, rendering,
  shading/depth assembly, progress checks, and partial-image snapshot creation.
  Publications are discarded by the diagnostic; it does not redraw the editor
  or upload each intermediate image.
- Mesh drawing uses the same retained path/tool/stock geometry and eight nearby
  camera angles. Native uses the normal GPU triangle renderer, waits for GPU
  completion, and performs no image readback. Browser uses Puri's CPU fallback.
  Native MSAA and the CPU fallback are different renderers, not exact-quality
  equivalents. Neither measurement includes the rest of the editor, Canvas2D
  image upload, Vello composition, display refresh, or event latency.
- For isolation, **all browser diagnostic work runs on its worker**, including
  preparation and CPU mesh drawing. The real editor still performs Grap
  preparation and mesh drawing on the page thread. A responsive timer during
  this diagnostic proves that the worker does not block the page; it does not
  prove that the complete editor sustains a particular frame rate.
- No browser startup/compile time, Safari comparison, cancellation latency,
  large-monitor scaling, or end-to-end interaction FPS was measured here.
- These are sequential runs on an active desktop, not a thermal-controlled lab.

## Reproduce

Builds remain under the repository's dependency-code sandbox:

```sh
./tools/sandbox-cargo build --release -p progred --features cam-profile --example cam_profile
./tools/sandbox-cargo web-threaded build --release -p progred --features cam-profile --example cam_profile
wasm-bindgen --target web --no-typescript --out-dir web/profile-pkg --out-name profile \
  target/sandbox/build-web/wasm32-unknown-unknown/release/examples/cam_profile.wasm
```

Run the already-built, headless native diagnostic with Metal access. The Cargo
build sandbox intentionally does not expose a GPU, so running this diagnostic
inside it fails to find an adapter. These commands open no application window:

```sh
target/sandbox/build/release/examples/cam_profile 0.02 512 4
target/sandbox/build/release/examples/cam_profile 0.5 512 4
node tools/profile-web-cam.cjs 512 4 0.02,0.5
```

Native arguments are `progress size trials [controls]`; browser arguments are
`size trials comma-separated-progress [controls|none] [worker-count] [package-directory]`. The optional `controls`
argument adds the backend comparison and substantially increases runtime.
The browser runner requires Playwright resolvable by Node and installed Google
Chrome. It serves only the diagnostic page, never calls `start_editor`, and
closes its own browser/server. Generated `web/profile-pkg` is ignored and does
not overwrite the editor's `web/pkg`.

## Follow-up: worker pool and WebGPU presentation

The same benchmark now uses `wasm-bindgen-rayon` for parallel Fidget computation,
and measures the existing native triangle renderer through browser WebGPU.
The application also uses the native Vello/image/mesh compositor in the browser;
no widget, Grap, or memo semantics changed. Browsers without a usable WebGPU
adapter retain Canvas2D and CPU mesh drawing.

Four trials per position, discard trial zero; GPU drawing waits for submitted
work to finish and discards each trial's first upload/draw. The browser GPU
measurement runs on the page thread after the worker returns shared geometry;
it does not encode vertices in JS or read back the image. CPU drawing remains
in the worker as a comparison. These drawing times omit editor/vector
composition, presentation, and event latency—not end-to-end FPS.

| Work | Earlier browser | Eight workers + WebGPU | Native baseline |
| --- | ---: | ---: | ---: |
| Stock mesh, 2% | 452 ms | 121 ms | 105 ms |
| Implicit image, 2% | 1,548 ms | 439 ms | 149 ms |
| Retained mesh draw, 2% | 52.8 ms CPU | 1.24 ms GPU | 1.59 ms GPU |
| Stock mesh, 50% | 9.03 s | 1.82 s | 1.35 s |
| Implicit image, 50% | 34.33 s | 6.40 s | 2.19 s |
| Retained mesh draw, 50% | 25.3 ms CPU | 0.84 ms GPU | 1.58 ms GPU |

The follow-up's CPU drawing remains 52.4 / 25.2 ms. The drawing win comes from
using the GPU, not making WASM's CPU rasterizer faster. Browser/native GPU
timing has different completion-notification overhead; do not interpret the
sub-millisecond difference as evidence the browser GPU itself is faster.
Fidget remains the VM backend on WASM. Native still benefits from its JIT.

Eight-worker linear memory peaked at 220,069,888 bytes (~210 MiB) across the
two positions, versus ~130 MiB for the single-worker baseline. This includes
worker stacks/allocator high-water marks and retained diagnostic geometry,
not live allocation size or browser/GPU total memory. Color/depth aggregates
and geometry counts match every baseline run. Maximum page timer gap was
13.33 ms across these eight trials; this is still not an editor-interaction
latency measurement.

The default pool is capped at eight rendering workers and leaves one reported
hardware thread free where possible. A four-worker midpoint comparison gave
5.02 s meshing / 10.03 s implicit, with ~159 MiB linear memory. Two workers
gave 6.27 s / 17.55 s and ~134 MiB. Eight workers were materially faster on
this machine. These runs are not a recommendation
to occupy every core on every browser/device.

Headless presentation checks use the actual browser backend, verifying vectors
below/above meshes, two clipped previews, curved and rectangular clips, partial
implicit depth replacement, CPU image upload, resize, and geometry replacement.
The same checks exercise forced Canvas2D fallback. They caught and fixed its
image upload's use of shared-memory typed arrays: `ImageData` requires a
non-shared JS-owned array. Normal WebGPU presentation does not take that path.

```sh
./tools/sandbox-cargo web-threaded build --release -p progred --example web_gpu_probe
wasm-bindgen --target web --no-typescript --out-dir web/gpu-probe-pkg --out-name probe \
  target/sandbox/build-web/wasm32-unknown-unknown/release/examples/web_gpu_probe.wasm
node tools/test-web-gpu.cjs
node tools/profile-web-cam.cjs 512 4 0.02,0.5 none 8
```

No editor was launched during these measurements. Visual interaction remains
for the user to test after rebuilding/reloading the website.
