# Browser orbit profiling — 2026-09-19

## Scope

Follow-up to the [component measurements](browser-native-profile-2026-09-19.md),
using the actual Command+9 document and installed orbit gesture handler. This is
an opt-in headless replay, not a launch of the editor application. Measurements
used Chrome on the same M3 Pro; **Safari was not measured**.

The editor is 2400×1800 physical pixels at scale 2, with equal-width preview and
source panes. The preview shows stock. Each animation-frame callback supplies
one pointer-motion batch, rebuilds the editor through its normal dispatch, and
paints through the browser's WebGPU compositor. Initial background work settles
before the replay; eight orbit frames warm up, then 80 frames are measured. A
separate 40-frame replay produces a Chrome CPU sample profile.

`update_ms` includes pointer handling and successor-frame construction;
`paint_ms` records the prepared drawing. `submit_ms` includes both plus CPU-side
graphics preparation/submission. It does **not** wait for GPU completion or
measure display latency. These are individual sequential trials on an active
desktop, not confidence intervals or an end-to-end FPS claim.

The replay deliberately does not poll worker publications during orbit. It
therefore measures contention from real background jobs, but omits the extra
frames caused by progress notifications in the application's Winit event loop.
It cannot rule out additional costs in that scheduling path.

## Foreground frame construction

Median timings in mesh-only mode, with no background work during orbit:

| Playback / source | Update | Paint | Total CPU submission | Submission p95 |
| --- | ---: | ---: | ---: | ---: |
| 2%, ordinary source | 15.46 ms | 0.35 ms | 17.07 ms | 18.00 ms |
| 50%, ordinary source | 15.19 ms | 0.35 ms | 16.75 ms | 18.13 ms |
| 50%, outline sections folded | 1.58 ms | 0.18 ms | 3.01 ms | 3.47 ms |

The folded control changes ordinary caller-owned collapse state, not the frame
algorithm. Stock and toolpath geometry remain the same. It made **zero mesh
vertex/index-buffer writes** over the measured orbit frames: camera changes
reuse the retained geometry. The refined-mode trials below also made zero such
writes. This does not mean zero GPU commands or uniform uploads.

In the ordinary-source midpoint CPU profile, about 51% of samples occur beneath
the source outline's projection/measurement, and about 14% beneath placement.
Allocator work overlaps those categories. Roughly 84% of samples occur beneath
frame rebuilding. This is not evidence that toolpaths are regenerated on orbit.

A native build of the same headless pointer/frame replay, at 50% with ordinary
source and no background rendering during orbit, measured 10.36 ms median update
(12.60 ms p95) and 0.24 ms paint. That native diagnostic records into `SplitCanvas`
without GPU submission, so only the update column is useful for comparison;
the hosts also use their ordinary, differing font configuration. The browser's
15.19 ms update is about 1.47× that native trial, not an orders-of-magnitude gap.

The source projection measures expanded content below the visible viewport as
well as visible content. That supplies layout and scroll geometry under the
current contract; it is not automatically an incorrect invalidation. Simply
skipping the source pane on orbit would introduce an event-specific cache, not
solve dependency tracking generally.

## Background contention

The refined preview starts new implicit work as the camera changes, cancelling
obsolete work through the normal latest-request machinery.

| Playback / background policy | Workers | Update | Total CPU submission |
| --- | ---: | ---: | ---: |
| 2%, ordinary refined preview | 1 | 20.91 ms | 22.47 ms |
| 2%, ordinary refined preview | 8 | 20.90 ms | 22.57 ms |
| 2%, new jobs held after initial render | 8 | 14.32 ms | 15.82 ms |
| 50%, ordinary refined preview | 8 | 67.74 ms | 69.85 ms |
| 50%, new jobs held after initial render | 8 | 17.72 ms | 19.40 ms |

The held-job control intercepts only the diagnostic page's outgoing job
messages after initial rendering has settled. Foreground preparation,
invalidation, cancellation, and latest-request replacement still execute.
Captured messages are delivered once before closing the isolated page. This is
a measurement control, **not** a proposed runtime scheduling policy.

At 2%, removing concurrent worker execution recovers the ordinary mesh-only
frame cost. At 50%, it reduces total CPU submission from 69.85 to 19.40 ms,
despite retaining the heap state from the completed initial implicit render.
One versus eight workers makes little difference in the early-playback trial, so
reducing the pool is not established as a solution. Cancelled jobs may spend
much of their time in serial preparation rather than parallel pixel work.

The shared-memory Rust standard library in our pinned nightly uses a **single
global `dlmalloc` protected by a spin lock**, shared by the browser thread and
workers. See the installed toolchain's
`library/std/src/sys/alloc/wasm.rs`, especially `lock::lock`. It explicitly avoids
blocking atomics because those are forbidden on the browser event-loop thread.
The ordinary-source midpoint profile has 295 of 1114 self samples in
`dlmalloc::{malloc,free}` and allocator entry functions; the refined midpoint
has 1010 of 2512 in those functions. More specifically, self samples in
`__rdl_alloc` / `__rdl_dealloc` rise from 32 to 812, consistent with contention
in their inlined lock acquisition. Sampling is not exact lock-wait accounting;
CPU scheduling and cache contention may contribute too.

## Worker preparation follow-up

The runner's optional `profile-worker` flag attaches Chrome's CPU profiler to
the computation coordinator (`worker.js`), separately from the page profiler.
Both profiles cover the same additional 40-frame replay, **after** unprofiled
timing and screenshot capture. This does not profile the nested Rayon workers.
No timers, counters, allocator hooks, or logging were added to production Rust.

Fresh default-allocator trials on Chrome 153.0.8010.48:

| Playback | Mean CPU submission | Median | p95 | Mesh buffer writes |
| --- | ---: | ---: | ---: | ---: |
| 2% | 23.77 ms | 23.75 ms | 26.33 ms | 0 |
| 50% | 56.46 ms | 71.29 ms | 75.75 ms | 0 |

The midpoint's mean is lower than its median because fast and stalled frames
alternate; see the [allocator comparison](browser-allocator-experiment-2026-09-19.md).
The following are **inclusive sample counts by stack ancestry**, not allocation
counts, exact phase timers, or percentages of all cores' execution:

| Coordinator stack | 2%: 1,723 samples / 1.11 s | 50%: 3,201 samples / 2.03 s |
| --- | ---: | ---: |
| `playback::Settings::remaining_stock` | 236 (13.7%) | 668 (20.9%) |
| `SoftwareScene::new` | 14 (0.8%) | 594 (18.6%) |
| `TreeOp::drop` | 325 (18.9%) | 1,627 (50.8%) |
| `partial::render` / `render_prepared` | 182 (10.6%) | 0 |
| `(idle)` | 949 (55.1%) | 257 (8.0%) |

The early trial's render samples were in a condition-variable wait while Rayon
worked, not pixels evaluated by this coordinator. Zero midpoint render samples
are consistent with cancellation before reaching rasterization; sampling cannot
prove that no brief render occurred between samples. Main-thread profiles still
show allocator contention. In the midpoint worker profile, 1,211 of the 1,627
tree-destruction samples are specifically in `__rdl_dealloc`, beneath
`TreeOp::drop`. These categories overlap; do not add them as separate costs.

Before the prepared-scene change below, the code explained this behavior:

1. `toolpath::fidget::computation::Request` includes camera-dependent raster
   settings plus the shared recording/playback settings.
2. Every changed camera request calls `scene(request)` in its worker, rebuilding
   the remaining-stock `Tree`, even when the recording and playback are unchanged.
3. `SoftwareScene::new` converts that tree into a deduplicated `Context`, SSA
   instructions, and register-allocated instructions. Cancellation is checked
   between objects, not during an individual object's import/compilation.
4. Once cancellation is noticed, the request-local stock tree still has to be
   destroyed. Its nodes are individual `Arc<TreeOp>` allocations. Cleanup cannot
   simply be cancelled without leaking memory.

This identified a stronger experiment than changing the global allocator:
separate camera-independent scene preparation from camera-dependent pixels,
using the existing explicit dependency graph. A camera edit should invalidate
pixels without rebuilding the stock expression or compiled root program. A
playback/geometry edit should invalidate preparation as well. This boundary is
implemented and measured in the follow-up below. It does not reuse
camera-dependent spatial specialization or imply that all remaining orbit work
is cheap.

### Would a job arena help?

Potentially. A worker can obtain a large block (or grow a small list of blocks),
suballocate its temporary storage without the shared allocator lock, and release
it in bulk. But an ordinary Rust `Vec`, `HashMap`, or `Arc` does not start using
that block merely because its caller has one. Those allocations need explicit
arena-aware representations/allocators. Transparently rerouting arbitrary
allocations would itself be a custom allocator, including ownership rules for
cross-thread frees and values escaping the job.

Keep temporary scratch separate from published images/meshes, which outlive the
job. Parallel tasks should have independent scratch regions rather than share
one new contention point. Explicit reusable vectors/maps are a simpler first
step where a few large buffers dominate; no unsafe slab is needed to retain their
capacity.

Fidget already does some of this: `Context` stores nodes in an indexed vector and
hash map, and voxel `Worker` retains coordinate arrays, evaluator storage, and
recycled specialized programs within a render. The input `Tree` is different:
it owns reference-counted nodes individually. A scratch arena for the rasterizer
alone would not remove the repeated stock-tree creation/destruction seen here.
First measure reuse of unchanged preparation; consider arenas only for remaining
necessary work. No arena or allocator change was made.

Raw artifacts are `target/orbit-worker-{early1,mid1}.{json,cpuprofile,png}` and
`target/orbit-worker-{early1,mid1}-worker.cpuprofile`. Decode the screenshot PNGs
to compare RGBA, rather than comparing compressed file bytes.

## Prepared-scene follow-up

The CAM image graph now has separate background scene-preparation and
pixel-rendering nodes. Preparation takes only the model objects or the
recording/playback stock recipe. It builds the stock expression and compiles a
shared immutable `SoftwareScene`. Pixel rendering takes that completed scene
and a separate bounds/camera/resolution request. Orbiting does not invalidate or
cancel preparation, including preparation already running; actual scene-input
changes do. The current-mesh prerequisite is retained. A pending scene never
supplies its previous compiled result to a new render request.

This uses the existing async memo/generation/cancellation mechanism. No custom
allocator, arena, new dependency, event-specific exception, or additional Fidget
patch was needed. Root tapes are retained, not camera-dependent spatial
specializations or per-job scratch buffers.

Same midpoint replay and Chrome version, sequential trials:

| Build | Mean CPU submission | Median | p95 | WASM memory before / after orbit |
| --- | ---: | ---: | ---: | ---: |
| Baseline, fresh repeat | 54.31 ms | 71.60 ms | 75.78 ms | 248 / 248 MiB |
| Prepared scene | 18.83 ms | 18.83 ms | 19.97 ms | 243.5 / 243.5 MiB |
| Prepared scene, repeat | 18.46 ms | 18.20 ms | 20.81 ms | 237.625 / 237.625 MiB |

At 2% playback the prepared-scene trial averaged 18.31 ms (median 18.33 ms,
p95 20.14 ms), compared with the earlier baseline's 23.77 / 23.75 / 26.33 ms.
WASM capacity stayed at 180.1875 MiB during that orbit. Early and midpoint
screenshots both match their baseline's decoded RGBA exactly.

The prepared-scene coordinator profile contains 1,346 samples: none beneath
stock construction or scene compilation, 319 beneath rendering (286 waiting
for Rayon), and 1,022 idle. The expensive repeated tree-construction/destruction
loop is gone from this orbit profile. This still does not measure aggregate
Rayon CPU time. All measured orbit frames made zero mesh-buffer writes. The
decoded replay screenshot matches the original midpoint baseline exactly.

The figure is CPU frame/submission time, not final-render throughput or
end-to-end visual latency. Initial rendering and geometry edits still pay the
preparation cost. The replay still omits progress-triggered editor rebuilds.
WASM capacity does not measure live allocations; these runs do not establish
a memory saving. Ordinary source projection remains a significant frame cost.

Artifacts: `target/orbit-preparation-baseline-mid2.*` and
`target/orbit-prepared-mid{1,2}.*`, including the first trial's `-worker.cpuprofile`.
The early-playback follow-up is `target/orbit-prepared-early1.*`.

Verification: 116 toolpath tests and 70 Fidget tests passed (manual diagnostics
remain opt-in). Coverage includes in-flight preparation surviving camera changes,
scene edits cancelling preparation, camera-request conflation, mesh-first ordering,
current failures, model-view reuse during tool motion, and exact image/depth
agreement with fresh compilation across multiple cameras and cancellation tokens.
The threaded-WASM diagnostic and ordinary web editor build successfully; the
normal `web/pkg` bundle was regenerated for manual testing. Native interactive
and Safari responsiveness have not been measured in this follow-up.

## Native prepared-scene comparison

A matched headless native replay gives a mixed result, not a general native
speedup. Same M3 Pro, 2400×1800 pixels at scale 2, expanded source, stock view,
native JIT, one computation coordinator, and the default Rayon pool (11 logical
CPUs). Input is paced at up to 60 Hz; each fresh process settles its initial
render, warms eight frames, then measures 80 frames. Three runs per variant and
position used the order before/after/after/before/before/after. The table pools
the 240 samples per variant.

| Playback | Before mean / p95 | Prepared-scene mean / p95 |
| --- | ---: | ---: |
| 2% | 22.31 / 53.21 ms | 10.52 / 11.69 ms |
| 50% | 11.80 / 13.93 ms | 31.00 / 49.46 ms |

Individual midpoint trial means were 11.20–12.44 ms before and 22.92–40.64 ms
after; the regression persists despite the variation. Early trial means were
17.82–25.32 ms before and 10.44–10.68 ms after.

These are CPU frame times: pointer handling, projection/layout, and draw recording
into `SplitCanvas`, including its destruction. They exclude GPU submission and
presentation, and the replay does not poll progress publications during orbit.
They are not directly comparable to the browser's CPU submission times or a
measurement of interactive native visual latency.

The baseline is HEAD `193c14d343a5569332f2dda5ef60ec9a0c5a570c`
with the identical profiling harness overlaid into an ignored source snapshot;
production preparation/render code is unchanged there. Both builds used the
repository Seatbelt Cargo wrapper and release configuration. No interactive app
was launched or working-tree production code reverted.

Bounded controls on the prepared-scene midpoint, changing only
`RAYON_NUM_THREADS` for the diagnostic process:

| Rayon workers | Mean / p95 frame | Initial setup, including render |
| --- | ---: | ---: |
| Default (11), three runs | 31.00 / 49.46 ms | 19.9–22.1 s |
| 4, one run | 14.04 / 15.94 ms | 28.8 s |
| 1, one run | 10.96 / 11.73 ms | 84.3 s |

This strongly suggests competition with background parallel raster/JIT work.
Retaining preparation lets an orbit request reach that work immediately;
previously repeated stock construction/compilation could be cancelled before
the request reached it. That explanation is an inference, not a native stack
profile establishing the exact contention site. Fewer workers are not a free
fix: the initial render becomes substantially slower. No production pool,
priority, or cancellation policy was changed. Investigate that native tradeoff
before treating prepared-scene reuse as a clean cross-platform checkpoint.

Raw data: `target/native-orbit-comparison.json` and
`target/native-orbit-worker-control.json`. Saved comparison executables are
`target/sandbox/native-orbit-{before,after}`. These are ignored local artifacts.

### Cancellation and quiet-period experiment

Temporary timestamp logging followed native midpoint orbit input through async
invalidation, cancellation signalling, and return from the worker computation.
The latter includes raster worker-local cleanup. There were 85 in-replay
cancellations after excluding the final frame to avoid mixing in shutdown:

| Interval | Median | p95 | Maximum |
| --- | ---: | ---: | ---: |
| Replay input to cancellation signal | 0.68 ms | 2.35 ms | 6.42 ms |
| Signal to computation return | 1.46 ms | 2.67 ms | 3.56 ms |

Including end-of-replay cancellation gives 87 cancelled jobs, with a 1.49 ms
median, 2.85 ms p95 and 5.77 ms maximum signal-to-return interval. This rules
against a long cancellation tail being the main issue **in this orbit replay**;
it does not bound cancellation during geometry preparation or other scenes.

A separate one-second native sampling profile captured 4,000 of 4,499 Rayon
worker stack samples under `JitTracingEval::eval` (about 89%). These workers were
executing generated interval code, not predominantly compiling it. The main
thread was primarily projecting/building the frame; the coordinator mostly
waited for Rayon. Thus the earlier mention of JIT compilation was only a
hypothesis: this sample instead supports competition with active parallel shape
evaluation. Sampling is separate from the timing comparison below.

A diagnostic-only 60 ms cancellable wait before pixel rendering tested avoiding
expensive starts while requests keep changing. No geometry or mesh work was
delayed. Two fresh-process trials per setting used the order 0/60/60/0 ms, with
the same default worker count and 80 measured frames each. Logging was disabled.

| Quiet period | Mean CPU frame | Median | p95 |
| --- | ---: | ---: | ---: |
| None | 25.82 ms | 23.88 ms | 42.46 ms |
| 60 ms | 10.86 ms | 10.71 ms | 12.39 ms |

Individual means were 23.98/27.67 ms without the wait and 11.12/10.59 ms with it.
This is a scheduling experiment, not a faster renderer: it deliberately gives up
implicit work during continuous input and adds roughly 60 ms before starting
after input settles. The existing mesh remains immediate. The arbitrary 60 ms
candidate is not an established optimum, and these tests still exclude GPU
presentation and progress-triggered frames.

The temporary wait occupied the coordinator with 2 ms sleep/check intervals to
isolate the effect; that is **not** a proposed production scheduler. A production
quiet-period option should retain the latest request and defer its submission
without occupying a computation worker, preserving ordinary cancellation and
generation validation. No debounce API or extra Fidget cancellation checks were
added on this evidence. All temporary logs and waits were removed afterward;
the native diagnostic was rebuilt without them. The web bundle was unchanged.

Artifacts: `target/native-cancellation-before.{log,json}`,
`target/native-cancellation-before-parsed.json`,
`target/native-cancellation-sample.txt`,
`target/native-cancellation-sampled.{log,json}`, and
`target/native-quiet-comparison.json`.

### Mouse-release submission gate

The adopted policy has no quiet-period timer: the CAM pixel worker waits while
the pointer button is held. Changed inputs still cancel obsolete work promptly;
release admits only the latest prepared request. Mesh updates and scene
preparation remain eligible during the drag. Pressing without changing the
render inputs retains a completed image or already-admitted valid work.
The generic async runtime accepts a separate tracked submission condition;
pointer policy stays in the CAM adapter. Waiting does not occupy a worker.

Two fresh native midpoint runs used the same default 11-worker pool, viewport,
expanded source, and paced replay as above. Neither builds nor other diagnostic
runs overlapped these measurements. The pooled 160 frames measured:

| Policy | Mean CPU frame | Median | p95 |
| --- | ---: | ---: | ---: |
| Immediate restart, preceding quiet-period control | 25.82 ms | 23.88 ms | 42.46 ms |
| Mouse-release gate | 12.44 ms | 12.34 ms | 14.21 ms |

Individual gated means were 12.13 and 12.74 ms. All 160 measured drag frames
reported zero outstanding background jobs. The replay then sent a real button
release through the installed input machinery: both runs started pending work
and completed the final implicit image (14.17 and 15.04 s, with publications
polled during this post-release phase). Initial setup was 18.63 and 19.65 s.
This verifies deferred work resumes, not just that dragging became cheaper.
As before, these frame timings exclude GPU submission and presentation and do
not establish browser or visible frame latency. This schedules expensive work
away from a drag; it does not make the renderer itself faster.

Regression tests cover held camera/playback changes, unchanged valid images,
latest-only release, readiness prerequisites, rejection of cancelled results,
inline completion, and pointer release/cancellation updating the tracked input.
Artifacts: `target/native-release-mid{1,2}.json`. `--finish` on the native replay
waits for the post-release render and reports its duration.

## Implications

- The GPU mesh handoff is working: repeated geometry uploads are not the
  bottleneck in these trials.
- Source projection/layout already uses most of a 60 Hz frame budget with idle
  workers. Any reuse should use general dependency-tracked boundaries, not an
  orbit-specific exception. Reducing transient allocation is also worth measuring.
- Shared-memory allocator contention is a browser-backend issue worth isolating
  before making architectural compromises in the core. A suitable allocator or
  reduced worker allocation may help. The prepared-scene change above removes
  unnecessary worker allocation without an allocator change. The one-worker
  result argues against blindly shrinking the pool.
  The subsequent [allocator experiment](browser-allocator-experiment-2026-09-19.md)
  found no clean win from Talc or a different lock around dlmalloc; neither was
  adopted.
- Live progress-event overhead, Safari, GPU completion, and visual smoothness
  still need separate verification.

## Reproduce

```sh
./tools/sandbox-cargo web-threaded build --release -p progred --features cam-profile --example orbit_profile
wasm-bindgen --target web --no-typescript --out-dir web/orbit-pkg --out-name orbit \
  target/sandbox/build-web/wasm32-unknown-unknown/release/examples/orbit_profile.wasm
node tools/profile-web-orbit.cjs 'position=0.5' baseline
node tools/profile-web-orbit.cjs 'position=0.5&collapsed' collapsed
node tools/profile-web-orbit.cjs 'position=0.02&refined&threads=1' refined-early1
node tools/profile-web-orbit.cjs 'position=0.02&refined&threads=8' refined-early8
node tools/profile-web-orbit.cjs 'position=0.02&refined&threads=8&hold-jobs' refined-held
node tools/profile-web-orbit.cjs 'position=0.5&refined&threads=8' refined8
node tools/profile-web-orbit.cjs 'position=0.5&refined&threads=8&hold-jobs' refined-mid-held
node tools/profile-web-orbit.cjs 'position=0.5&refined&threads=8&profile-worker' worker-mid1
node tools/profile-web-orbit.cjs 'position=0.02&refined&threads=8&profile-worker' worker-early1
# Preserve a baseline bundle separately when comparing code changes:
wasm-bindgen --target web --no-typescript --out-dir web/orbit-prepared-pkg --out-name orbit \
  target/sandbox/build-web/wasm32-unknown-unknown/release/examples/orbit_profile.wasm
node tools/profile-web-orbit.cjs 'position=0.5&refined&threads=8&profile-worker&package=orbit-prepared-pkg' prepared-mid1
# Native frame/paint recording only (no GPU submission or window):
./tools/sandbox-cargo build --release -p progred --features cam-profile --example orbit_profile
target/sandbox/build/release/examples/orbit_profile
target/sandbox/build/release/examples/orbit_profile --refined --position 0.5
target/sandbox/build/release/examples/orbit_profile --refined --position 0.5 --finish
RAYON_NUM_THREADS=4 target/sandbox/build/release/examples/orbit_profile --refined --position 0.5
```

The runner needs Playwright and installed Chrome. It starts its own loopback
preview server with `--no-open`, closes its browser/server afterward, and writes
raw samples and DevTools-loadable `.cpuprofile` files under ignored `target/`.
Generated bindings are ignored under `web/orbit-pkg/` and the candidate package
directories. Diagnostic commands do not update the ordinary web bundle. After
verification, rebuild that bundle through `make build-web`; the worker-count
policy is unchanged.
