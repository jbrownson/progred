# Browser Fidget bulk-evaluation experiment — 2026-09-19

## Result and status

A bounded rewrite of Fidget's common bulk-float arithmetic loops reduced the
midpoint CAM implicit render from **8.14 s to 5.78 s (29% less time)**. Meshing
fell from **2.33 s to 1.63 s (30%)**. Final color/depth fingerprints matched.

After the investigation, the measured change and its tests were applied to the
vendored core. The browser editor uses it, as does native meshing; the native
Apple Silicon implicit JIT path is unchanged. A
[standalone patch](experiments/fidget-core-bulk-slices.patch) preserves the exact
change for upstream review. Nothing has been submitted upstream.

## Profile the workers, not the waiting coordinator

Baseline: `bbfc3b70`, after the browser SIMD flag change. One warmed, completed
Cmd+9 benchmark was sampled through Chrome's CPU profiler at 1 ms intervals.
The coordinator's DevTools target auto-attached its eight nested Rayon workers;
all nine profiles were captured. Stack ancestry separated implicit rendering
from the preceding mesh computation.

Approximate distribution of active implicit-render worker sample time:

| Work | Share |
| --- | ---: |
| Bulk float evaluator | 49.7% |
| Interval evaluator | 41.0% |
| Tape simplification, including register allocation | 7.6% |
| Gradients | 0.9% |
| Other | 0.9% |

These are aggregated worker samples, not percentages of elapsed wall time.
Waiting samples were excluded from this table. Allocation and publication were
not the main costs. Profiled runs were not used for the timing comparison.

## Change

The old loop repeatedly accesses nested vectors as
`slots[out][i] = slots[lhs][i] + slots[rhs][i]`. The output register may be one
of the inputs. The generated WASM float-evaluator bodies contained no SIMD
arithmetic even with `+simd128` enabled.

The experiment handles those register-equality cases once per instruction,
then loops over safely borrowed slices. `get_disjoint_mut` covers distinct
registers; explicit in-place loops cover overlapping ones. Unary and binary
helpers handle common arithmetic, square root, square, negation, absolute
value, and register/register min/max. Other operations are untouched.

LLVM now emits `f32x4` add/subtract/multiply/divide/square-root instructions in
those evaluator bodies (34 such instructions per inspected specialization,
versus zero before). This does not require hand-written SIMD, unsafe code,
new allocation, fast-math, changed sampling, or a new cache. Min/max still call
Fidget's original operations, including their NaN and tie behavior.

## Measurements

Apple M3 Pro, macOS 27, Chrome 153.0.8010.48, eight Rayon workers. The existing
headless `cam_profile` runs the real 6,588-segment fixture without launching the
editor. Both versions enable WASM SIMD. The render retains the existing tile
sizes, four-times depth sampling, geometry, mesh depth, and camera.

For 512², run order was candidate A → baseline → candidate B, three trials at
each position per batch. The table uses all three baseline and six candidate
trials. The 100% check uses two trials each at 256² and should be treated as a
smaller correctness cross-check, not equally strong performance evidence.

| Playback / image | Operation | Baseline median | Candidate median | Less time |
| --- | --- | ---: | ---: | ---: |
| 2% / 512² | Implicit | 512 ms | 232 ms | 55% |
| 2% / 512² | Mesh | 156 ms | 88 ms | 43% |
| 50% / 512² | Implicit | 8.14 s | 5.78 s | 29% |
| 50% / 512² | Mesh | 2.33 s | 1.63 s | 30% |
| 100% / 256² | Implicit | 5.26 s | 3.59 s | 32% |
| 100% / 256² | Mesh | 3.31 s | 2.36 s | 29% |

Midpoint implicit ranges were 7.98–8.50 s baseline and 5.50–6.15 s candidate.
Excluding the first trial at each position in each batch still gives about 27%
less midpoint implicit time. These are Chrome measurements, not Safari or
native speedup claims. Native Apple Silicon implicit rendering uses the JIT,
not this evaluator; native meshing does use it, but was not benchmarked here.

## Correctness

Every recorded comparison had matching final image and depth hashes, occupied
pixel counts, and stock mesh vertex/triangle counts:

| Fixture | RGBA fingerprint | Depth fingerprint |
| --- | --- | --- |
| 2% / 512² | `705e1f9dcd106553` | `6f0dc9e04155606f` |
| 50% / 512² | `12b1e29a303a80a0` | `d4ff62c840076406` |
| 100% / 256² | `4a659f9e1612be2d` | `d26b58abd005df53` |

The candidate includes a regression covering every input/output register alias
combination, empty and non-vector-multiple lengths, untouched trailing data,
signed zeros, infinities, and NaNs. Fidget's core test suite passed with the
candidate: **268 tests, zero failures**. The ordinary browser editor bundle was
subsequently rebuilt with this patch; native apps pick it up on their next build.
The integration check also passed all core, JIT, and raster library tests:
**483 tests, zero failures**.

## Full-size preview follow-up

The user reported about a minute at the boundary between op1 and op2 in Chrome.
The 5.78 s figure above was not an estimate for that case: it measured only the
implicit stage at 512² physical pixels and 50% of total cutting distance.

A read-only inspection of the open editor found a 1512×806 CSS-pixel canvas,
3024×1612 backing pixels, and device-pixel ratio 2. The preview occupied about
one third of the width, below the menu: roughly 1000×1550 physical pixels.
The startup log confirmed WebGPU presentation and eight render workers.

A temporary headless diagnostic evaluated both operations independently:

- Op1: 5,448 segments, cutting distance 166.01588586905478.
- Op2: 1,140 segments, cutting distance 46.63081352817123.
- Combined: 6,588 segments, cutting distance 212.64669939722654.
- The operation boundary is thus at distance progress **0.78071226282677986**,
  not 0.5. The normal UI uses leaf positions; this converts that boundary for
  the diagnostic's distance-based progress input.

With the optimization applied, the same fixture, default diagnostic camera,
eight workers, and one trial at each resolution:

| Op1/op2 boundary | Implicit stage | Meshing | Whole diagnostic |
| --- | ---: | ---: | ---: |
| 512×512 physical pixels | 7.52 s | 2.17 s | 10.36 s |
| 1024×1536 physical pixels | **63.43 s** | 2.09 s | 66.50 s |

The larger case approximates the user's pane, not an exact replay of their
camera or saved document. Nevertheless, the renderer alone reproduces the
reported minute-long wait; editor scheduling overhead is not needed to explain
it. These are sizing checks, not another baseline/candidate speedup comparison.

The app derives image size from pane size times display scale. The base depth
is image height times `max(zoom, 1)`, rounded up to 64; the final pass multiplies
depth by four. These two tests therefore use depths 2048 and 6144. The taller
view also enlarges the projected model because framing is height-based: occupied
pixels increased from 41,213 to 370,236. Runtime is adaptive, not a simple count
of every voxel in this grid, but it is substantial additional work.

The browser diagnostic now accepts `WIDTHxHEIGHT` as well as a single square
size. No app rendering quality, resolution, or scheduling was changed. The
temporary operation-boundary diagnostic was removed after extracting the data.
Raw logs: `target/cam-op-boundary.log`, `target/cam-op-boundary-512.log`, and
`target/cam-op-boundary-1024x1536.log`.

## Reproduce

The patch is already applied in this checkout. To reproduce the comparison,
generate a baseline diagnostic package from `bbfc3b70` and compare it to the
patched checkout. Build through the repository sandbox, without changing flags:

```sh
./tools/sandbox-cargo web-threaded build --release -p progred --features cam-profile --example cam_profile
wasm-bindgen --target web --no-typescript --out-dir web/profile-simd-pkg --out-name profile \
  target/sandbox/build-web/wasm32-unknown-unknown/release/examples/cam_profile.wasm
node tools/profile-web-cam.cjs 512 3 0.02,0.5 none 8 profile-simd-pkg
node tools/profile-web-cam.cjs 256 2 1 none 8 profile-simd-pkg
node tools/profile-web-cam.cjs 1024x1536 1 0.78071226282677986 none 8 profile-simd-pkg
./tools/sandbox-cargo test --release -p fidget-core --lib
```

Retain a separately generated baseline package before building the patched code.
Both packages must sit directly under `web/`, because generated bindings import
the worker host from their parent directory. Run timing batches serially, with
no builds or other profiling jobs competing with them.

Ignored local artifacts: `target/completed-cam-baseline*.{json,cpuprofile,log}`,
`target/profile-completed-cam.cjs`, `target/bulk-slices-*.log`, and the retained
baseline diagnostic package under `target/profile-bulk-baseline-pkg`.
