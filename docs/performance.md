# Frame performance checks

These opt-in tests measure headless frame construction; ordinary builds and
launches contain none of their instrumentation. Results recorded since October
2026 are in the [performance log](performance-log.tsv). Earlier measurements and
investigations live in the [September log](history/performance-log-2026-09.md)
and the other dated notes indexed in [history](history/README.md).

## Tracking performance

Performance can slip a little at a time, and a change can affect it somewhere
unexpected. `make perf` runs the canaries below and compares them with the
latest recorded run on the same machine, built with the same Cargo features:

```sh
make perf                                  # three rounds of every canary
make perf ARGS='--editor'                  # the canaries an editor change drives
make perf ARGS='--only fidget --rounds 1'  # a quick subset, by test name
make perf ARGS='--record "what changed"'   # also append the results to the log
make perf ARGS='--history'                 # recorded medians on this machine
make perf ARGS='--features cam-profile'    # a build with progred features
```

Two habits keep this cheap:

- **A weekly check.** Run every canary and record it,
  `make perf ARGS='--record "weekly check"'`. A Claude Code session-start hook,
  [`tools/perf-check-due`](../tools/perf-check-due), says when the latest
  recorded run is more than a week old. Before investigating what a check flags,
  confirm it with interleaved runs of the flagged canaries at the last recorded
  commit and at the current one on the same machine.
- **Per change, only when it plainly matters.** A change to evaluation, layout,
  rendering, Fidget, CAM, or a dependency compares the canaries it drives
  before and after, such as `--editor` for an editor change, without recording
  them. Most changes need no run.

It builds the progred library tests with the `perf` profile, the release build
made incremental, and runs each canary serially. After a function-body edit
it rebuilds in about 16 s on a 4-core VM, where a release build took two to four
minutes; the first build takes about six. Runs recorded before 17 October 2026
used the release profile, so the first weekly check after it may differ from
them by the profile alone.

Each canary reports its headline timing on a `perf` line, through
`crate::perf::record`; the tool names the workload after the test, adding a
metric name when a test reports several. Over several rounds it keeps each
workload's lowest median, with that round's p95, and reports the spread of the
medians as a percentage of the lowest. A change from the last record is flagged
when it exceeds 5% or twice either run's spread. Frame canaries measure 60
frames, and the four voxel orbits 12. The sub-millisecond canaries measure for
about a third of a second on a fast machine instead: 600 forest and CAM
controls frames, 1,500 pairs of layout FFI frames and 20,000 color picker
frames. Sixty of their frames end a few milliseconds after the test starts,
before the operating system has settled where it runs: on a laptop with
performance and efficiency cores, a change that only shortened their setup made
them read 20–30% slower, and within 3% once they ran longer. Single-shot
workloads, such as stock meshing and the software render of the CAM example's
fully cut stock, report one sample per round.

[`performance-log.tsv`](performance-log.tsv) has one row per workload per
recorded run:

- **commit**: with `+` when tracked files had uncommitted changes;
- **machine**: the hardware, which decides what is comparable, with the
  processor's family, model and stepping on a virtual machine;
- **gpu**: the adapter Fidget's implicit previews select;
- **environment**: the OS version, power source on macOS, Rust version, and
  any Cargo features the build enabled;
- **median_ms** and **p95_ms**, the measured **samples** per round, the
  **rounds**, and their **spread_pct**;
- a **note** on what changed or why the run was recorded.

Compare runs only on the same machine and adapter, and keep the environment in
mind: a different power state, OS, or compiler can account for a change. Even on
one Mac, three default runs in October 2026, on battery, on AC power after two
release builds, and on AC power idle, spread by up to 18% on the heavy
workloads, so a smaller difference on a stock or Fidget workload is drift, not a
change. A shared cloud VM is noisier: its rounds varied by 10–20%, so use one
for interleaved comparisons within a session, and prefer a dedicated machine for
the weekly record when there is one. Its processor name can be masked as well:
in October 2026, two VMs named `Intel(R) Xeon(R) Processor @ 2.80GHz` five days
apart ran the projection canaries 25–70% apart, a gap that interleaved runs of
their two commits on one machine didn't show. So on a VM, the machine also names
the processor's family, model and stepping.

The tool runs the test executables directly, outside the build sandbox, so on a
Mac the voxel orbits use Metal, as the app does. A machine without a GPU may
offer a software adapter such as Mesa's llvmpipe; the voxel orbits then measure
it, not the CPU fallback. The two GPU compositor canaries run only on a hardware
adapter, or with `--gpu`. Implicit rasterization in software uses Fidget's VM on
every platform. A run with features compares with earlier runs with the same
features, or with the default build until there is one.

To add a canary, record its headline timing with `crate::perf::record` and add
the test to the suite in [`tools/perf.py`](../tools/perf.py).

## Headless editor captures

The SVG exporter can capture a whole editor frame, including document views,
panes, dividers, text, and embedded PNG images, without opening a window:

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  editor_svg_captures -- --ignored --nocapture
```

This writes `editor_fidget.svg`, `editor_fidget_cube.svg`, and `editor_toolpaths.svg` into
`target/sandbox/build`. Each SVG is self-contained; raster images retain their
transforms, transparency, and enclosing clips. The test helper accepts an editor
state and window size and paints through the normal frame pipeline into a
`DrawList`. OS window chrome and native menus are not included. PNG encoding and
base64 are test-only dependencies.

Fidget uses its normal backend selection; without GPU access in the build
sandbox it uses CPU rendering, so lighting may differ from the native GPU view.
These are layout/content captures, not pixel-exact Vello screenshots: glyphs
use unhinted outlines, and the existing vector exporter supports solid brushes
only. Open the SVGs in a browser, or convert them to PNG with `rsvg-convert`
(from librsvg):

```sh
rsvg-convert target/sandbox/build/editor_fidget.svg \
  -o target/sandbox/build/editor_fidget.png
rsvg-convert target/sandbox/build/editor_fidget_cube.svg \
  -o target/sandbox/build/editor_fidget_cube.png
rsvg-convert target/sandbox/build/editor_toolpaths.svg \
  -o target/sandbox/build/editor_toolpaths.png
```

Quick Look thumbnails can crop wide SVGs rather than preserve the viewport;
use the SVG or the conversion above when checking the whole frame.

The progressive CAM capture uses a worker thread and the normal completion-driven
frame updates, saving each observed image size as `cam_progressive_N.svg`:

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  editor_toolpath_progressive_svg_captures -- --ignored --nocapture
```

It reports elapsed times through headless painting, including earlier capture
overhead, not native display latency. It also checks increasing resolution and
that the final image fills the pane behind the controls. In one optimized run
at 1000 × 750 @1, the 333 × 750 CAM pane showed 42 × 94 at 239 ms,
84 × 188 at 503 ms, 167 × 375 at 919 ms, and native pixels at 1629 ms.
These are a smoke check of the checked-in example, not a performance guarantee.

## Frame timing

The opt-in tests in `progred/src/projection/tests/frame/profile.rs`, and the
layout library's FFI canary, share one headless frame harness,
`progred/src/projection/test_support.rs`. They are not assertions about
interactive frame rate, and ordinary builds and launches contain none of this
instrumentation.

`make perf` prints only each canary's headline timing. To see the phases
below, run the canaries through Cargo, serially in an optimized build:

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  profile_loop -- --ignored --nocapture --test-threads=1
```

Use `fidget_orbit_profile_loop`, `iop_tree_profile_loop`,
`iop_tree_source_profile_loop`, or `color_picker_profile_loop` as the filter to
isolate a workload. The default
is five warm-up frames followed by 60 measured frames. Override the measured
count through Cargo configuration, since the build sandbox clears shell
environment variables:

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  --config 'env.FRAME_PROFILE_ITERATIONS="180"' \
  fidget_orbit_profile_loop -- --ignored --nocapture --test-threads=1
```

## Inputs and measurements

Two additional opt-in checks cover layout construction:

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  grap_layout_ffi_profile_loop -- --ignored --nocapture --test-threads=1
./tools/sandbox-cargo test --release -p progred --lib --features layout-profile \
  iop_source_form_profile -- --ignored --nocapture --test-threads=1
```

The first alternates the two variants each pair: 20 two-text rows built by Grap
as GID layout descriptions versus scoped layout-emitting FFIs. Both use the
same explicit fuel budget, library stack, fonts, text cache, geometry, and draw
endpoint. A normal regression test compares their complete paint commands and
extents, with and without the existing border combinator. No timing assertion
is a correctness test.

The workload was reduced from 100 to 20 rows on 2026-09-23: the returned layout
program runs with its own ordinary fuel allowance, not the larger explicit
allowance used to construct it. The old 100-row case exhausted that allowance
and rendered an error presentation instead of a layout (its output assertion
caught this). A regression checks both exhaustion and successful execution with
a larger allowance. The 100-row timings in the
[September log](history/performance-log-2026-09.md) are not directly comparable
to the current smaller fixture; application fuel behavior has not changed.

The `layout-profile` feature adds diagnostic scopes and a test-only counting
allocator. Ordinary builds have neither the scopes nor that allocator. The
report partitions **exclusive** elapsed time and allocation/reallocation
requests by the currently executing scope. Bytes are requested allocation sizes,
not retained or peak memory. These timings include instrumentation overhead;
use the feature-free canaries for baseline frame times. Constructors and
measurement are tagged by construct, but later placement, hover, and painting
are phase totals, not attributed back to each originating widget. In particular,
`Program` means native projection recursion/adaptation, **not Grap execution**.
The same instrumented harness also accepts `color_picker_form_profile` to
measure an open RGBA picker; `_form_profile` runs both workloads.

`BenchFrame` takes a document, source-qualified root path, selection,
annotations, available width, placement origin, clipping rectangle, and pointer.
`BenchContext` retains the library stack, fonts, layout context, text shaping,
and caller-owned computation graph. A `ProfileView` configures logical size and display scale, using zero
padding for viewport declarations and the editor's normal margin for a document
view. The `profile` combinator takes a frame-producing closure and an output
check. Other documents and state sequences can reuse it without changes to the
libraries or runtime.

Each iteration rebuilds the projected view, measures and places it, and runs its
paint continuations into a headless `DrawList`. The report separates:

- First-frame time, including any lazy initialization. This is **not** necessarily
  cold shader compilation: the OS may already have compiled shaders on disk.
- Warm median, p95, and maximum total time, including output disposal.
- Preparation (projection, text metrics, and choice-graph construction), choice
  resolution plus settled geometry, placement/hover, after-hover binding,
  painting plus handler disposal, and final output disposal. These timers live
  only in the test harness. Library work occurs in its normal phase: Fidget
  renders during preparation, while the IoP canvas program runs during painting.

Fixture parsing and app-lifetime resource construction are outside the timer.
Per-frame input construction is included; output validation is excluded. There
are no timing thresholds in unit tests. Compare repeated runs on the same
machine, power state, document, viewport, and scale factor; use interleaved
before/after runs when evaluating small changes.

## Current workloads

| Workload | Logical size | Scale | Input sequence |
| --- | --- | --- | --- |
| IoP picture | 500 × 500 | 1 | Rebuild the declared viewport |
| IoP source | 1400 × 900 | 1 | Project the document's top clipped viewport |
| RGBA picker | 600 × 400 | 2 | Project a selected color with its picker open |
| Fidget orbit | 400 × 600 | 2 | Advance yaw by 2° per frame; pitch 60°, zoom 1 |
| Torus orbit | 400 × 600 | 2 | Same camera sequence |
| Tanglecube orbit | 400 × 600 | 2 | Same camera sequence |
| Gyroid sphere orbit | 400 × 600 | 2 | Same camera sequence |
| Fidget cube orbit | 400 × 600 | 2 | Same camera sequence; remeshes the Rhino-derived cube at depth 5 |
| Toolpath orbit | 400 × 600 | 2 | Same camera sequence; compensated cutter paths, Fidget stock subtraction/meshing, and playback controls |

The additional filters are `fidget_torus_profile_loop`,
`fidget_tanglecube_profile_loop`, `fidget_gyroid_profile_loop`,
`fidget_cube_profile_loop`, and `fidget_toolpaths_profile_loop`. These use the
same helper as the original Fidget canary, changing only the document. Their
editable formulas and sources are described in [the examples guide](../examples/README.md).

The cube and toolpath fixtures now use [mesh previews](fidget-mesh.md), not voxel
rendering. The cube canary remeshes every frame; the toolpath canary retains its
dependency-tracked geometry and records mesh drawing without GPU rendering, which
`cam_mesh_roundtrip_profile` covers separately. That canary compares the direct
composite with one through an intermediate image and allows 0.1% of pixels to
differ by more than two levels: the two resolve edge coverage differently along
the mesh's silhouette. The toolpath sink generates tube
triangles directly, without implicit path fields. Earlier
voxel timings are not unchanged baselines. Other Fidget canaries still use the voxel path
described below.

### Interpreting viewport measurements

Fidget receives ordinary camera annotations at the viewport's source path. It
uses the library's normal automatic backend: GPU when available, CPU fallback
otherwise. Its assigned 800 × 1200 viewport (less controls where present) is
rendered through the real preview projection,
including field evaluation, Fidget lowering, rendering, and image construction.
When the GPU path runs, its synchronous readback is included too. The harness
does not identify which backend was used, so do not label these results as GPU
timings without separately establishing that. No GPU permissions are added to
the build sandbox. Every frame checks that a correctly sized image was produced,
not a fast fallback projection of an error. A small ordinary test also verifies
that changing the camera actually changes the pixels.

The common endpoint is a `DrawList`, **not** Vello execution or presentation.
The tests exclude the rest of the editor window, event delivery, input batching,
display synchronization, and compositor latency. They cannot detect a recurrence
of the event scheduling bug where several expensive drag updates ran between
paints; keep the interactive orbit/scroll check as a separate test of smoothness.

Blob storage has a separate opt-in microbenchmark:

```sh
./tools/sandbox-cargo test --release -p gid --test blob_profile -- \
  --ignored --nocapture --test-threads=1
```

It compares owned vectors, shared slices, shared vectors, and a test-only
64-byte sharing threshold. Construction, clone/disposal, construction plus
four clones, and editing a shared snapshot are separate workloads. Each size
rotates implementation order over seven rounds and reports median nanoseconds
per operation; allocation of the source fixture is outside measurement.

Older dated reports used different timing scopes and document margins; start a
new baseline rather than directly comparing their averages to these medians.
