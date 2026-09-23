# Frame performance checks

The [multi-operation CAM rendering investigation](cam-render-profiling-2026-09-15.md)
separates stock, tool, path, mesh and progressive-render costs, checks job reuse,
and records expression-grouping experiments and a tested local CPU JIT fix.
The JIT roughly halves full-stock implicit time with its recommended tiles,
at a cancellation-latency cost; meshing does not improve. The application uses
the patched JIT for implicit rendering on Apple Silicon macOS, retaining the VM
for meshing and other platforms.

The opt-in [Fidget meshing experiment](fidget-meshing-2026-09-13.md) measures
CPU triangle generation from the cube document, independently of frame rendering.

## CAM pane resizing — 2026-09-18

The divider drag exposed a misplaced computation boundary: the viewport's
size-dependent closure also built the program tree. Width and height entered
the environments of generated cutting functions, invalidating path recording
even though the resulting cuts were unchanged. A diagnostic at successive
widths measured about 6 ms rebuilding the tree/control declaration and
263–265 ms recording the same paths. Equal recording results could preserve
downstream geometry, but only after that synchronous work had already run.

The example now uses the viewport declaration's optional `prepare` function to
build its tree without dimensions. Its tracked application has the same
definition/effect observation rules as existing evaluation memos; the ordinary
viewport function receives the prepared result plus width and height.

The release regression below measures preparation, controls-declaration
construction, and recording demand at four changed sizes: 18–72 µs after the
initial build. It asserts that resizes retain the exact tree storage and
recording result, then changes tilt and checks that both invalidate. This is
not a whole-frame or input-to-display measurement: layout, mesh presentation,
and camera-sized implicit images still have their normal resize costs.

```sh
./tools/sandbox-cargo test --release -p progred --lib cam_resize_retains -- --nocapture
```

## Uncached CAM program construction — 2026-09-17

The 504-leaf, five-level toolpath example exposed repeated expansion of shared
runtime containers and closure environments when converting results to GID.
The runtime already shared those values; the conversion did not. Separately,
`with controls` converted each argument before constructing its result.

It now retains runtime arguments until a GID boundary, and that conversion
preserves existing shared subgraphs. A conversion-local address table remembers
every traversed runtime container and environment; it is not a cross-evaluation
cache or content interner. Reference counts cannot identify every repeated visit:
different closure environments can share an outer frame that owns a container
only once. Shadowed environment bindings are not materialized. Closure capture
remains unchanged.

Run the explicitly uncached canary with:

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  profile_program_tree_construction -- --ignored --nocapture
```

Five local release runs before/after measured:

| Computation | Before | After |
| --- | --- | --- |
| Program tree alone | 62–94 ms | 5.3–7.0 ms |
| Tree plus controls/view declaration | 226–255 ms | 5.1–6.7 ms |

The second measurement includes tree construction; these times are not
additive. They measure direct `grap::apply`/`evaluate`, with no computation memo,
Fidget geometry, rendering, or editor frame. They are not whole-frame latency
claims. The declaration-to-projection controls boundary remains in place.

The follow-up shared-outer-environment regression exposed a missed case in the
reference-count shortcut. Removing that shortcut preserves those containers too.
Local uncached warm samples moved from roughly 5–7 ms to 6–8 ms for construction;
remembering all traversed allocations adds bookkeeping. Memo-hit demands still
take a few microseconds and do not materialize the result again. This is a
sharing-correctness tradeoff, not another measured orbit speedup.

The same canary also times deriving path-based hierarchy selectors from the
504-leaf tree. After the selection-intent change, five samples took 33–83 µs.
This includes building keyed hierarchy/row descriptions, not placement or paint.

### Owned-result experiment — 2026-09-17

An experimental `evaluate_owned`/`apply_owned` API retained runtime results and
their shared code storage beyond one evaluation, with explicit GID conversion.
The API, supporting evaluator changes, benchmarks, and trial app migration were
removed: neither tested consumer justified the extra ownership, storage-lifetime,
and cross-evaluation capability machinery. Existing within-evaluation lowered
values and sharing-preserving GID conversion remain.

Five local release trials built the 504-leaf program tree in 4.2–4.9 ms, then
materialized it in 1.8–2.7 ms. Recording all leaves was slower through retained
results: 351–400 ms versus 314–335 ms through GID, with matching segments and
fuel use. Adding the API did not establish a meaningful regression in the
existing GID construction canary, which remained around 6–9 ms.

A controls/presentation migration initially raised the controls-only frame
median from 0.128 ms to 2.23 ms by materializing a captured program tree just to
inspect nested lists. Direct runtime-list inspection removed that cost, but
three paired release runs still measured retained callbacks at 0.147, 0.151,
and 0.157 ms versus GID callbacks at 0.130, 0.137, and 0.138 ms. Each sample used
five warm-up frames and 60 measured frames including disposal, with 3D work
replaced by a placeholder. First-frame timings were not compared; the cases
ran in fixed order and shared initialization. These are headless canaries,
not native orbit latencies or a statistical study.

The existing memoized GID result already shares large data and avoids repeating
conversion on unchanged frames. Reconsider retained runtime results only with
a demonstrated consumer benefit, measuring the receiving computation rather
than conversion savings alone. Avoid materializing whole closure environments
merely to inspect container structure.

### Runtime callback handoffs — 2026-09-23

The new owned runtime representation now serves a concrete consumer: closures
returned through presentation, controls, layouts, drawing programs, and event
handlers retain their code origins. The forest hover regression is enabled and
finds the innermost available displayed call. Inline evaluated expressions are
anchored to their original occurrence; creator call stacks are not retained.
Existing GID-oriented projections still use explicit conversion adapters.

Local optimized canaries compare:

| Version | Warm uncached tree construction | Controls-only frame median |
| --- | --- | --- |
| Before owned results, `15985033` | 7.6–8.5 ms | 0.305–0.309 ms |
| Ownership checkpoint, `9c676235` | 9.2–10.8 ms | 0.332–0.341 ms |
| Runtime consumer migration | 9.5–11.4 ms | 0.333–0.345 ms |

The final three frame medians were 344.58, 332.75, and 339.29 µs. Each uses five
warm-up frames and 60 measured frames including disposal, with 3D work replaced
by a placeholder. Two final construction runs each had five samples; the first
sample of each is excluded from the warm range. Memo hits took about 2–3 µs.
Builds and other test runs were finished before these final measurements.

This is approximately unchanged from the ownership checkpoint, not a recovered
speedup. Against the pre-ownership baseline there remains roughly 20–40%
uncached-construction overhead and 8–13% controls-frame overhead. These short
local runs are not a statistical study, native orbit latency, browser performance,
or Fidget rendering measurements. The earlier baselines were measured before
the consumer migration rather than interleaved with it. Source continuity is the
demonstrated benefit; more conversion removal remains possible.

Use `profile_program_tree_construction` and `cam_controls_profile_loop` with
`--release --ignored --nocapture --test-threads=1`. Keep separate build outputs
for archived source copies: sharing a target directory during the earlier
comparison reused incompatible artifacts.

### Runtime projection inspection — 2026-09-23

Fold classification, collapsed display/picking, source highlighting, text/blob
partials, and empty partials no longer request whole-runtime-value conversion.
Runtime f64 record inspection reads only the numeric field, including when
unrelated metadata contains closures. Expanded structural display and other
legacy partials still request GID views.

The same release canaries measured a controls-only frame median of 317.54 µs
before this pass and 329.75, 324.58, and 328.71 µs afterward (five warm-ups,
60 frames per run). Warm uncached construction was 9.05–11.58 ms before and
9.79–10.76 ms after; memo hits stayed around 1–3 µs. These sequential local
measurements demonstrate no speedup, and do not recover the ownership overhead
above. This pass removes unnecessary conversion requests rather than proving
an end-to-end performance improvement. The remaining adapters can still
materialize the same value later in projection dispatch.

### Repeated-frame regression

The uncached construction improvement alone missed a frame-level regression:
the `with controls` constructor lacked its tracked-read declaration. This made
the surrounding evaluation memo rerun each frame, then compare freshly rebuilt
closure environments with the previous result. A CPU sample attributed about
22% of the headless viewport test to that result comparison, versus about 5%
to the evaluation itself (most remaining time was the CPU mesh rasterizer).

The constructor now declares its reads tracked. This uses the existing memo;
controls and view callbacks still run each frame, and effectful or untracked
arguments still prevent reuse. A regression test covers reuse, unrelated edits,
changed inputs, and both kinds of non-reusable argument.

The same 400×600 @2 headless viewport test fell from a 108.9 ms median to
79.7 ms. This test uses the CPU triangle fallback, not the app's GPU mesh path,
so these are not native input-latency figures. A separate
`cam_controls_profile_loop` retains the viewport, declaration memo, controls,
and view-call evaluation but replaces the final 3D projection with a placeholder.
After the fix, that pipeline measured 0.127 ms median / 0.155 ms p95 over 60
warm frames. The construction canary now also measures repeated demands of one
stable memo root; these took 2.6–8.3 µs after the first evaluation.

### Native orbit: GPU mesh buffer packing

A 20-second sample of the running editor during orbiting found a separate
main-thread hotspot in the GPU mesh adapter: nested byte iterators packing
vertices, and flattening padded image-readback rows. This path is not exercised
by the headless CPU raster benchmark above. The app was using GPU triangle
rendering; the expensive work was CPU-side copying around that render.

The adapter now reserves the exact output capacity and appends component bytes
and complete rows with slice copies. No geometry cache, unsafe casts, scheduling,
or rendering-policy change is involved. Tests compare output bytes, including
signed zero and NaN payloads, and verify readback padding removal.

The isolated `mesh_gpu_packing_profile` benchmark uses 500,000 vertices and
1,600 readback rows (5,000 pixel bytes plus 120 padding bytes each). Initial
release measurements were about 22 ms → 3.5 ms for vertices and
4.2 ms → 0.4 ms for readback copying. These are synthetic packing costs, not
app frame times. An index-packing loop was also tried but was slower than the
existing iterator, so that code is unchanged. Run the benchmark without a GPU:

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  mesh_gpu_packing_profile -- --ignored --nocapture
```

### Remaining orbit comparison

After the packing fix, compare equivalent workloads before attributing a small
subjective difference to the evaluator or controls. The pre-hierarchy example
at `29b83063` starts playback at 35%; the hierarchy starts at zero, so more
upcoming paths are visible. With the current code and both fixtures set to zero,
the headless mesh-orbit canary measured 75.9 ms versus 77.0 ms median over 20
frames. The old fixture at its original 35% measured 58.2 ms. These are CPU
fallback numbers, not native GPU/input-latency measurements; background implicit
work and native presentation pacing are excluded.

Separately, with ordinary initial folds, `cam_source_profile_loop` measured
6.9 ms for the old example versus 7.9 ms for the current example at 600×900 @2.
The controls-only canary measured 0.077 ms versus 0.121 ms. Both comparisons run
the same current executable against the two fixtures, isolating example growth
rather than claiming a complete old-binary/new-binary comparison.

The orbit canary now discovers the preview's actual occurrence through a
test-composed partial and asserts that camera updates change the image. Its
previous one-result-step assumption missed the CAM controls/render wrappers,
so older CAM measurements above rendered a fixed camera despite their orbit
label. They still measured repeated-frame work, not actual changing-camera cost.

`CAM_PROFILE_SOURCE` optionally chooses a fixture for the CAM source, controls,
and orbit canaries. Supply benchmark variables through Cargo configuration:
the sandbox wrapper intentionally clears inherited environment variables.
Keep comparison fixtures under `target/sandbox` where the test can read them.

```sh
./tools/sandbox-cargo --config 'env.FRAME_PROFILE_ITERATIONS="20"' \
  --config 'env.CAM_PROFILE_SOURCE="/absolute/path/to/target/sandbox/before.gid"' \
  test --release -p progred --lib fidget_toolpaths_profile_loop -- --ignored --nocapture
```

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

### Implicit stock quality and cancellation investigation — 2026-09-14

The opt-in `implicit_stock_diagnostics` and `implicit_normal_sampling_diagnostic`
tests in `libraries/fidget/raster/diagnostics.rs` exercise the checked-in Grap
toolpath's stock subtraction without a window. Run with the same optimized sandbox test command and
`--ignored --nocapture`. The stock test writes `stock_*.png` captures.

Two runs at 600 × 600, camera zoom 1.2, 0.125-inch diameter and 0.22-inch length
measured stock-expression construction below 1 ms and compilation at 8–20 ms
for playback 0.35, 0.7 and 1.0. Native stock rasterization ranged from roughly
0.29 to 1.8 seconds. These isolate stock; they omit future-path/tool scene
construction and editor work. They do not establish total interactive latency.

The stock captures had no transparent gaps within occupied scanlines and no
zero/nonfinite normals. Extra Z samples reduced jagged shading seams; rendering
at twice X/Y resolution and box-filtering the shaded colors reduced aliasing.
The latter diagnostic is a simple byte-space average, not the final production
antialiasing policy. Neither observation identifies an arbitrary user screenshot.

Fidget's pinned software renderer checks cancellation between top-level tiles,
not inside their recursive evaluation. With the fully cut stock, its default
`[128, 64, 32, 16, 8]` tiles rendered in 1.28 s and returned about 1.24 s after
a cancellation requested 20 ms into rendering. `[64, 32, 16, 8]` took 0.73 s
and 0.29 s respectively; `[32, 16, 8]` took 0.66 s and 18 ms. All three PNGs
were byte-identical. These are local measurements, not bounds on cancellation.
The outer request still rejects cancelled results even when Fidget finishes
an already-running tile without observing its token. Smaller tiles use an
existing public Fidget setting; no upstream modification is necessary.

The normal diagnostic confirmed a separate integration defect: Fidget returns
gradients in sampling coordinates. Our software shading originally normalized
them without accounting for unequal sampling-axis scale. A fixed `x + z` plane
changes from RGB 158 to 172 as resolution changes, although its physical normal
is constant. Dividing gradient components by the lengths of the corresponding
screen-to-model matrix columns recovers the same normal at every tested level.
This is a shading correction, not a repair for missing geometry.

The subsequent `implicit_tool_rim_diagnostic` isolates the user's cap/side seam
with the actual ball-end tool expression (0.125-inch diameter, 0.22-inch length).
It compares Fidget pixels against analytic ray intersections with the flat cap
at 300 × 300, pitch 40 degrees. Of 9,635 rays strictly inside the cap rim, native
depth sampling missed 39 and gave another 857 a non-cap normal. At 4× depth
these counts were 17 and 194; at 16× depth they were 2 and 55. Captures are
`tool_rim_z1.png`, `tool_rim_z4.png`, and `tool_rim_z16.png`. They reproduce the
sawtooth rim without other scene objects. Initial captures used uncorrected
shading; rerunning uses the current corrected lighting.
These distinguish two finite-depth-sampling effects: a ray can traverse a thin
cap/side wedge between samples without an inside sample, or its first inside
sample can select the side's gradient rather than the cap's. The earlier stock
scanline check did not exclude either behavior at the tool rim.

Splitting those analytic cap intersections into near/far halves confirms that
missing coverage is only the far-side problem in this fixture. At the current
4× depth, the near half has zero missing pixels and 98 wrong-normal pixels;
the far half has 17 missing pixels and 96 wrong-normal pixels. The near-side
sawteeth are shaded cylinder-wall normals, not holes in the solid tool.

### Software raster fixes and final depth refinement — 2026-09-14

Production software shading now removes sample-axis scale from gradients;
the fixed-plane diagnostic returns RGB 173 at every tested resolution.
Regression tests cover camera rotation, zoom, rectangular images and the final
depth pass. Software rasterization uses `[32, 16, 8]` tiles through Fidget's
existing public setting. A same-process full-stock comparison measured 1.22 s
with default tiles and 0.56 s with the smaller tiles. The resulting PNGs are
byte-identical, and an ordinary test compares the two paths on a clipped sphere.

CAM requests one additional native-size pass with 4× depth sampling. The native
image is published first, while that pass runs; only the last result clears the
pending indication. The multiplier is an explicit software-refinement argument,
not a rule in the async scheduler. Depth refinement preserves the camera,
render volume, X/Y coordinates and normal weighting. No supersampling or
post-render gap filling is applied. Tests cover preserved geometry, final output,
and cancellation after the native image but before the finer-depth pass.

An optimized 1000 × 750 headless-editor run published 42 × 94 at 208 ms,
84 × 188 at 395 ms, 167 × 375 at 567 ms, native 333 × 750 at 871 ms,
and the same-size 4×-depth image at 1612 ms. These include prior capture overhead
and are not native display timings or a controlled comparison with earlier runs.
The capture test now records depth-only updates even though dimensions repeat.

At 600 × 600, stock-only rasterization with the new tiles took 0.20–0.53 s
across playback 0.35, 0.7 and 1.0; 4× depth took 0.42–1.25 s. Cancellation
requested 20 ms into rendering returned after 7–204 ms over 18 native/finer-depth
trials, most in tens of milliseconds. Default tiles in the same run delayed it
1.19 s. These remain cooperative checks between root tiles, not a latency bound;
expression construction and compilation still check only around their work.

## Frame timing

The opt-in tests in `progred/src/projection/tests/frame/profile.rs` share one
headless frame harness. They are not assertions about interactive frame rate,
and ordinary builds and launches contain none of this instrumentation.

Run the current canaries serially in an optimized build:

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

The first alternates the two variants each pair: 100 two-text rows built by Grap
as GID layout descriptions versus scoped layout-emitting FFIs. Both use the
same explicit fuel budget, library stack, fonts, text cache, geometry, and draw
endpoint. A normal regression test compares their complete paint commands and
extents, with and without the existing border combinator. No timing assertion
is a correctness test.

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
rendering. Their current canaries include CPU meshing every frame, triangle
rasterization, and native upload/readback when the GPU is available. The toolpath
sink generates tube triangles directly, without implicit path fields. Earlier
voxel timings are not unchanged baselines. Other Fidget canaries still use the voxel path
described below.

A 2026-09-13 sandboxed run of the mesh toolpath canary on the M3 Pro measured
40.03 ms median (44.22 ms maximum) across eight frames after five warm-up frames,
at an 800 × 1200 raster. This includes Grap path generation, direct tube mesh
construction, cube remeshing at depth 5, and **CPU triangle rasterization**;
Seatbelt exposed no GPU adapter. The first frame was 50.88 ms. These are
headless viewport-build timings, not native GPU or input-to-display latency.

After adding playback and Grap surface-normal compensation later that day, the
same sandbox canary measured 66.34 ms median (70.03 ms maximum), first frame
78.31 ms, with five warm-up and eight measured frames. This is a changed workload:
it includes the extra Grap mapping, an evaluation-local seekable recording, cutter
and stock geometry, and a measured control strip that reduces the raster height.
It still uses CPU rasterization. It is not an isolated measurement of slider
overhead and does not predict native GPU interaction latency.

With the now-removed heightfield stock-removal experiment enabled (160 grid cells per XY
axis across the then-1.1-unit stock, progress 0.35), the same headless canary later measured 63.23 ms median
(66.26 ms maximum), first frame 79.95 ms, again five warm-up/eight measured
frames with CPU rasterization. This includes rebuilding the stock and its mesh
on every frame. The small difference from the preceding 66.34 ms run is not an
isolated speedup claim; it shows no obvious overall regression in this workload.
Native GPU responsiveness still needs an interactive check.

The fixture subsequently changed to one-inch stock and now omits the reference
solid while stock is enabled, avoiding coplanar surfaces. The historical timings
above include reference-cube meshing that this mode no longer performs.

### Volumetric stock subtraction — 2026-09-14

The heightfield experiment is replaced by ordinary Fidget box-minus-sweep
expressions. The same M3 Pro sandbox canary (400 × 600 logical @2, playback 0.35,
five warm-up and 60 measured frames) measured:

| One-inch stock, no reference solid | Median | p95 | Maximum | First frame |
| --- | ---: | ---: | ---: | ---: |
| Heightfield, before replacement | 50.09 ms | 54.79 ms | 59.90 ms | 70.56 ms |
| Fidget subtraction, mesh depth 7 | 256.91 ms | 272.24 ms | 311.57 ms | 283.39 ms |

Both use CPU triangle rasterization because the sandbox has no Metal adapter.
This compares usable implementations, not equal geometric approximations:
Fidget can represent roofs and through-cuts, and depth 7 was chosen because
depth 5 visibly distorted the narrow grooves. Completed path lines are now
omitted. At this checkpoint every expression, stock mesh, and image was
recomputed; the later dependency-graph comparison below supersedes that policy.

To separate the geometry costs from rasterization:

```bash
./tools/sandbox-cargo test -p progred --release stock_meshing_profile -- --ignored --nocapture
```

One isolated run measured the following at depth 7. Each row is a single sample,
not a median; meshing includes octree construction and dual-contour extraction.
Fixture parsing, library setup, and Grap path generation are outside these
columns (51.90 ms together in that run).

| Playback | Build stock expression | Compile | Mesh + extract | Triangles |
| --- | ---: | ---: | ---: | ---: |
| 0% | 0.012 ms | 0.019 ms | 41.31 ms | 24,630 |
| 35% | 0.333 ms | 8.17 ms | 218.18 ms | 61,266 |
| 100% | 1.03 ms | 20.80 ms | 323.48 ms | 66,178 |

The dominant cost is CPU meshing, not constructing the sweep expressions.
Native GPU triangle drawing will not remove that cost. Remeshing every frame
is therefore expected to be choppy; retained geometry and asynchronous work
remain separate design decisions, not hidden fallbacks in this experiment.

### Dependency-tracked CAM geometry — 2026-09-14

These measurements used the earlier mesh fixture. Command+9 now automatically
refines its mesh fallback with implicit images. Its default mesh depth is 6 rather than 7,
and the cutter diameter is now 0.125 inches; keep these geometry changes in mind
when comparing fresh measurements with this table.

The same `fidget_toolpaths_profile_loop` (400 × 600 logical @2, playback 0.35,
five warm-up and 60 measured orbit frames) before/after the general memo graph:

| Configuration | Median | p95 | Maximum | First frame |
| --- | ---: | ---: | ---: | ---: |
| Recompute geometry each frame | 259.83 ms | 283.33 ms | 348.41 ms | 267.27 ms |
| Dependency-tracked recording, stock, mesh | 12.22 ms | 12.58 ms | 12.82 ms | 271.14 ms |

Both runs used the CPU triangle rasterizer because the build sandbox exposes no
Metal adapter. This is roughly 21× faster warm orbiting, not faster meshing or
an input-to-screen latency measurement. The image changes with the camera and is
rendered every frame. Geometry edits and first demand still pay synchronous
recording/meshing cost. See [the boundaries and limitations](incremental.md).

A repeat measured 12.41 ms median / 12.90 ms p95 (286.19 ms first frame).
After refactoring the example so CAM consumes the cube's surface and normal
functions, the same canary measured 12.13 ms median / 12.79 ms p95
(276.27 ms first frame). Geometry-parameter regression tests also verify that
cached paths invalidate on edits and equal freshly evaluated paths.
The unchanged, uncached IoP canaries measured 26.12 ms for the picture and
3.76 ms for source in this pass; there was no fresh pre-change IoP A/B run,
so these are canary observations, not a quantified regression comparison.

The synchronous toolpath orbit canary explicitly selects `preview paths mesh`
in its copy of the current fixture. It must not run the automatic renderer with
an inline executor and mistake synchronous completion of the entire implicit
refinement sequence for interactive orbit cost.

After separating stock meshing from path appearance and tightening memo failure
recovery, the final run measured 12.51 ms median / 13.27 ms p95 / 14.26 ms maximum,
with a 285.26 ms first frame. A regression test verifies that path color/thickness
changes retain the identical shared stock-mesh result; playback and depth changes
replace it.

### Automatic mesh/implicit handoff — 2026-09-15

The headless `editor_toolpath_refined_svg_captures` test uses the production
Command+9 document at 1000 × 750 logical @1 (333 × 750 viewport). Work is queued
explicitly and executed on a worker; mesh painting uses the CPU triangle backend
because Seatbelt exposes no Metal adapter. Before restoring intermediate implicit
resolutions, one run measured an 8.30 ms full-editor
orbit frame using the retained mesh, a current native-XY implicit image after
358 ms, and final four-times-depth after 1155 ms. Playback's first frame took
6.76 ms with the moved tool and desaturated previous stock, followed by 245 ms
of meshing. These are single-run timings, not native input-to-display latency
or a before/after speedup claim.

Camera/framing tests compare both renderers, including rectangular views and zoom.
The software normal conversion now also reverses sample Y into the upward camera
axis used by triangle lighting, avoiding a light-direction change at handoff.

The standalone mesh-orbit canary on the current fixture measured 10.29 ms median,
10.75 ms p95, and 11.26 ms maximum over 60 frames after five warm-ups; its inline
first frame took 221.67 ms. Fixture/depth changes prevent treating differences
from the older depth-7 tables as a speedup attributable to this composition.

### Background CAM stock — 2026-09-14

The generic async boundary preserves the warm-orbit canary: 12.55 ms median /
13.04 ms p95 / 13.64 ms maximum. This canary deliberately uses the inline
headless executor, so its 297.26 ms first frame still includes all geometry work.

The separate `editor_toolpath_async_svg_captures` check uses the full editor at
1500 × 1050, a controlled queue, and an actual worker thread. It renders a frame
before allowing each queued stock job to run. One release run measured:

| Stage | Frame build and headless paint | Separate stock job |
| --- | ---: | ---: |
| First pending frame | 72.77 ms | 223.49 ms |
| First ready frame | 11.78 ms | — |
| Playback changed from 0.35 to 0.70, old stock visible | 9.78 ms | 282.46 ms |
| Replacement stock ready | 10.03 ms | — |

This verifies that the responding frame does not wait for meshing, not native
input-to-display latency or performance under worker/UI CPU contention. Initial
Grap path generation remains synchronous. The sandbox used CPU rasterization;
SVG serialization is excluded. The test writes first/ready/updating/updated
captures, including the stale-stock color treatment, without opening a window:

```bash
./tools/sandbox-cargo test -p progred --release editor_toolpath_async_svg_captures -- --ignored --nocapture
```

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

## Scoped layout FFIs and construct profile — 2026-09-07

Two interleaved, feature-free runs of 180 measured frames (five warm-up pairs)
on the same local ARM64 Mac:

| 100 two-text rows | Run 1 median | Run 2 median |
| --- | ---: | ---: |
| Grap builds GID layout, then decode | 0.366 ms | 0.372 ms |
| Grap emits native layout through scoped FFIs | 0.321 ms | 0.333 ms |

That is 10–12% less complete-frame time on this synthetic workload. Preparation
alone improved about 4–7%; disposal and later phases also contribute to the
difference. The timing includes creation/reification and application of the one
ordinary layout-program closure, so it does not hide that boundary's cost.
It is not a 10–12% improvement to the editor: existing source projections are
already native Rust, and these examples have not been rewritten to use the new
FFIs. This test establishes the cost of the optional Grap construction path.

The ordinary canaries remained comparable to the preceding pass: IoP source
4.35 ms median (4.53 ms p95), picture 22.61 ms (23.73 ms p95). The five Fidget
canaries also passed: original 8.84 ms, torus 6.69 ms, tanglecube 43.13 ms,
gyroid 28.53 ms, cube 14.01 ms median. These retain the headless sandbox's
automatic backend and exclude Vello/presentation, as described above.

The separately instrumented IoP source run had about 105,600 allocation/reallocation
requests per frame. Its largest exclusive buckets were:

| Scope | Instrumented time/frame | Time share | Allocation requests/frame |
| --- | ---: | ---: | ---: |
| Native recursion/adaptation (`Program`) | 1.107 ms | 22.6% | 26,610 |
| Projection construction | 0.710 ms | 14.5% | 30,626 |
| Hover and handler assembly | 1.181 ms | 24.1% | 12,179 |
| Placement | 0.415 ms | 8.5% | 11,692 |
| Shared-node preparation overhead | 0.309 ms | 6.3% | 2,078 |
| Choice resolution and geometry | 0.299 ms | 6.1% | 5,833 |
| LineEdit construction/preparation | 0.114 ms | 2.3% | 3,304 |

Other scopes account for the remainder. LineEdit's later placement/hover work is
included in those phase totals, not in its 2.3%. This does not justify a special
lowered LineEdit constructor by itself. The stronger next leads are the numerous
short-lived path/target/layout allocations during projection and adaptation,
and the general continuation/output assembly in placement and hover. Profile
those call sites before changing representations; no such optimization is part
of this pass.

## Shared list-position bytes — 2026-09-07

Tracing the allocation-heavy path handling found that every `Position` clone
copied a `Vec<u8>`, including each list step in a copied path. Positions now
share immutable bytes with `Arc<[u8]>`; equality, hashing, ordering, and the
path encoding still use the bytes, not allocation identity. This adds no cache,
interner, or invalidation. New positions convert their construction buffer to
shared storage once; clones no longer allocate, and GID remains Send/Sync.

Feature-free source runs used the same two saved release test executables,
under the Cargo sandbox, in A/B, B/A, A/B order. Each run used five warm-up
frames and 180 measured frames at 1400 × 900, scale 1:

| Source frame, including disposal | Pair 1 | Pair 2 | Pair 3 |
| --- | ---: | ---: | ---: |
| Copied position bytes | 4.32 ms | 4.36 ms | 4.34 ms |
| Shared position bytes | 4.07 ms | 4.07 ms | 4.10 ms |

That is about 6% less frame time. Preparation fell from about 2.14 ms to
1.96 ms; final output disposal fell from about 0.115 ms to 0.079 ms.
The separately instrumented run counted 105,597 versus 91,857 allocation/
reallocation requests per source frame: 13,740 fewer (13%). Most of that
reduction was in recursion/adaptation (26,610 → 17,056).

All eight canaries also passed in A/B and B/A order at 60 measured frames.
The IoP picture remained about 22.3–22.6 ms; the Fidget CPU-fallback measurements
varied slightly in both directions. This is a source construction/disposal
improvement, not a claimed speedup of drawing or GPU rendering. No change to
hover composition or projection callbacks was included in this experiment.

## Initial baseline — 2026-09-06

Local ARM64 Mac, macOS 26.6.2, release build under Seatbelt. Five warm-up frames
then 60 measured frames, run serially:

| Workload | First frame | Warm median | Warm p95 | Warm maximum |
| --- | ---: | ---: | ---: | ---: |
| IoP picture | 23.29 ms | 23.01 ms | 23.62 ms | 24.51 ms |
| IoP source | 22.71 ms | 4.52 ms | 4.66 ms | 4.74 ms |
| Fidget orbit, CPU fallback | 17.79 ms | 9.63 ms | 10.98 ms | 11.59 ms |

A separate one-second stack sample of a longer Fidget run confirmed execution
in `fidget_raster::voxel::render` and its CPU workers. **These are not native GPU
preview timings.** A separate 2,000-frame run had an 8.62 ms median, 10.47 ms p95,
and 27.90 ms maximum, illustrating why one short run isn't a precise promise.
The sampled run was deliberately stopped afterward and is not used for timing
statistics. Local sampling evidence is in
`target/sandbox/tmp/fidget-frame-profile-2026-09-06.sample.txt` (not committed).

The remaining gap is a GPU-capable profiling runner, with an explicit rendering
backend report, separate from the restricted Cargo build environment. Until
then this Fidget canary covers projection/lowering and CPU rendering only on
this setup. Do not relax the build sandbox or treat these numbers as a check of
Metal, readback, Vello, or interactive scheduling.

## Additional Fidget examples — 2026-09-06

Same machine and sandbox, all four documents at 400 × 600 points, scale 2,
five warm-up frames and 60 measured frames:

| Document | First frame | Warm median | Warm p95 | Warm maximum |
| --- | ---: | ---: | ---: | ---: |
| Original Fidget | 9.14 ms | 8.71 ms | 10.16 ms | 10.68 ms |
| Torus | 8.80 ms | 6.69 ms | 7.53 ms | 8.00 ms |
| Tanglecube | 45.35 ms | 46.57 ms | 55.38 ms | 71.17 ms |
| Gyroid sphere | 42.09 ms | 28.74 ms | 35.62 ms | 39.99 ms |

These use the same CPU-fallback setup as above, not the application's GPU
backend. The tanglecube costs more than the visually denser gyroid here; surface
appearance or operator count alone does not predict rendering cost. Keep all
four as distinct workloads rather than treating their names as a performance
ordering.

After adding the Fidget arithmetic source projections, the same IoP source
canary measured 4.56 ms median and 4.73 ms p95 (60 frames), consistent with the
earlier 4.52 ms baseline. This is a check for unrelated dispatch overhead, not
a before/after measurement of the Fidget documents themselves.

After the projection-scope and missing-value changes, the 1400 × 900 @1 IoP
source canary measured 3.65 ms median and 3.80 ms p95 (60 frames). The preceding
run before default picker selection and name suggestions was 3.62 ms median;
this shows no material regression, not a claimed speedup. This source-only
canary has no active selection and does not measure picker interaction latency.

## Layout builder and placement consolidation — 2026-09-07

The native layout frontend is now a reusable program over `Builder`, with a
production choice-graph builder and a test recorder. The chosen graph places
directly; `Measured` no longer contains another container enum. Alternative
selection policy and per-frame sharing are unchanged. This completes the
native representation consolidation, not a direct Grap-to-native-layout FFI
bridge: stored Grap layout forms still decode from GID.

Local ARM64 Mac, same restricted release harness as above. Immediately before
this pass, 60-frame canaries measured IoP source at 4.57 ms and picture at
23.72 ms median. An early post-change run measured 4.86 / 24.18 ms; after the
final borrowed-leaf cleanup, an isolated 60-frame source repeat was 4.35 ms.
The final seven-workload run used 180 measured frames:

| Workload | Median | p95 | Maximum |
| --- | ---: | ---: | ---: |
| IoP source | 4.36 ms | 4.52 ms | 4.76 ms |
| IoP picture | 22.38 ms | 22.86 ms | 23.77 ms |
| Fidget orbit | 8.65 ms | 9.82 ms | 10.33 ms |
| Torus orbit | 6.76 ms | 7.53 ms | 8.91 ms |
| Tanglecube orbit | 42.95 ms | 51.16 ms | 95.88 ms |
| Gyroid sphere orbit | 28.33 ms | 35.78 ms | 41.28 ms |
| Fidget cube orbit | 13.84 ms | 16.20 ms | 16.89 ms |

These small before/after differences are not a controlled speedup claim. There
was no interleaved checkout A/B, and the short runs vary enough to change the
sign of the difference. The earlier continuation and bracket changes also
changed costs and line breaks. This pass does not establish that the entire
refactor is faster than the original implementation. Fidget numbers remain
sandbox CPU-fallback canaries, not measurements of interactive Metal rendering.

IoP source phase medians from that final run:

| Phase | Median |
| --- | ---: |
| Preparation | 2.14 ms |
| Choices and settled geometry | 0.29 ms |
| Placement | 0.40 ms |
| Hover and handler construction | 1.18 ms |
| Paint and handler disposal | 0.23 ms |
| Output disposal | 0.12 ms |

The picture spends 22.28 ms in paint/handler disposal; all preceding phases
together take about 0.01 ms. Each Fidget viewport spends essentially its entire
time in preparation, which includes the actual Fidget render. Neither points
to alternative search as the picture bottleneck.

A separate five-second, 1 ms stack sample of a 2,000-frame source run found
about 35% of the benchmark thread's samples ending inside the system allocator
(allocation, freeing, and resizing). This is a conservative count of reported
allocator leaf symbols, not a separately additive phase. Copying/clearing memory
also features prominently. Application self-time includes temporary builder
result consumption, definition lookup, settled geometry, and hover output
ordering (`Fragment::reverse_since` and `controls_below`). Optimized symbols and
inlining limit finer attribution. The sample run overlapped a compile briefly
and is not used for timing comparisons.

The actionable distinction is construction/disposal versus layout search:
preparation and hover/handler construction dominate the source frame, while
choice resolution is about 7%. Reusable layout closures, captured paths,
handlers, and hover scoping still allocate. Fewer representations do not by
themselves remove those costs. Investigate those allocations before changing
the search policy or adding caches. No runtime profiling or new cross-frame
state was introduced.

For a repeatable CPU sample, start `sample` waiting for the headless test
executable's process name, then run the source filter with
`--config 'env.FRAME_PROFILE_ITERATIONS="2000"'`. Use that run for call stacks
only and take clean wall-time measurements separately.

## Hover-output borrowing and empty handlers — 2026-09-07

Baseline: `318341c`, including shared list-position bytes. A temporary finer
breakdown counted 7,305 hover callbacks and 1,124 mapped output scopes per IoP
source frame. The latter isolate child output, restore its public ordering, and
merge it back. A separate five-second CPU sample also found buffer reversal,
control composition, memory movement, and allocation/freeing in this phase.
The extra scopes substantially perturb timing, so their times are not used as
speedup measurements. They were removed after the investigation.

Two small changes preserve the continuation and ordering contracts:

- `HoverContext` borrows the accumulating `Fragment`, instead of taking and
  restoring the complete output for every callback. Its lifetime is confined to
  the callback; paint and event continuations still own their captures.
- `Handler` represents its empty identity explicitly. Combining a function with
  an empty handler returns that function directly, without allocating another
  composition closure. Ordinary declining handlers are not treated as empty.

Feature-free release binaries were saved separately and alternated in
before/after/after/before/before/after order, with five warm-up and 300 measured
frames per run, at 1400 × 900 logical pixels and scale 1:

| Metric | Before | After |
| --- | --- | --- |
| Whole-frame medians across three runs | 4.07 / 4.07 / 4.07 ms | 3.96 / 3.95 / 3.96 ms |
| Hover + handlers, median | 1.14 ms | 1.03–1.04 ms |
| Paint + handler disposal, median | 0.214–0.216 ms | 0.194–0.195 ms |
| Hover-phase allocations per frame | 12,179 | 10,911 |
| All allocations per frame | 91,857 | 90,589 |

This is about a 3% whole-source-frame improvement, not a large architectural
speedup. The handler-only experiment was much smaller: before medians
4.06 / 4.09 / 4.03 ms versus 4.00 / 4.03 / 4.03 ms, mostly saving disposal work.
The allocation reduction comes from removing 1,268 empty-handler composition
closures; borrowing the output removes moves, not allocations.

All eight canaries were also run in before/after/after/before order with 60
measured frames each. IoP picture medians were 22.30 / 22.24 ms before and
22.49 / 22.12 ms after; the Fidget CPU-fallback and layout-FFI canaries likewise
showed no consistent regression. These workloads do little hover construction,
so this change is not expected to materially accelerate their rendering.

Mapped child output still has buffer/reversal costs. Avoiding all of those
would require changing how arbitrary output transformations are represented;
this pass deliberately leaves that alone. Alternative selection, hover
precedence, clipping, navigation, and optional painting are unchanged. No cache,
partial invalidation, or runtime instrumentation was added.

## Direct delimiter painting — 2026-09-07

Baseline: `637b5eb`. Delimiters still built a one-command `Drawing` during
hover, then interpreted it during paint. Temporary diagnostic scopes counted
830 delimiter constructions, about 0.38 ms and 3,452 allocations per IoP source
frame. Only 304 of those delimiters reached painting: the ordinary leaf clip
had already discarded the other paint continuations, but their outlines had
been built anyway.

Delimiter measurement still reports the same advance and minimum span. Its
paint continuation now constructs the outline and calls the canvas directly,
without a drawing-command vector or interpreter. The backend still receives
the same Bezier path. Square brackets also extend that path from rectangle
iterators rather than allocating two temporary paths. Geometry, brush,
transforms, hover, and clipping policy are unchanged. Skipping paint skips
outline construction naturally; there is no additional visibility test or cache.

Feature-free release binaries were alternated in before/after/after/before/
before/after order, with five warm-up and 300 measured frames per run, at
1400 × 900 logical pixels and scale 1:

| Metric | Before | After |
| --- | --- | --- |
| Whole-frame medians across three runs | 4.07 / 4.05 / 3.99 ms | 3.71 / 3.70 / 3.70 ms |
| Hover + handlers, median | 1.05–1.07 ms | 0.658–0.662 ms |
| Paint + handler disposal, median | 0.195–0.201 ms | 0.317–0.319 ms |
| All allocations per frame | 90,589 | 87,941 |

The whole-source-frame improvement is about 8–9%. Some time intentionally
moves from hover into paint; the whole-frame numbers include both phases and
disposal. The rectangle-iterator change alone is small: direct-paint medians
3.71 / 3.75 ms versus 3.68 / 3.75 ms with the iterators. It removes another 208
allocations per frame without changing the path elements.

All eight canaries also ran in before/after/after/before order with 60 measured
frames. IoP picture medians were 22.95 / 23.01 ms before and 22.64 / 22.46 ms
after; this does not indicate a meaningful picture-rendering speedup. Fidget
CPU-fallback timings fluctuated across runs, as before; those viewports do not
exercise the changed delimiter painter. These are headless measurements, not
on-screen frame rates. The temporary finer profiling scopes were removed.

The 19 SVG outputs exercised by the headless projection tests were compared
with the saved baseline. Fifteen are byte-identical; the sample and its pending
variant differ only in glyph outlines for newly minted short cell IDs. All
delimiter and other non-text geometry is identical. Workspace tests, native
all-target checks, and the web target check pass.

## Direct color controls and image leaves — 2026-09-07

Baseline: `21692b7`. Native color controls built command vectors, mapped their
brushes into semantic paints, then interpreted them. The alpha checkerboard
and markers also built temporary vectors. They now call `CanvasSink` directly,
through one deferred painter per control. A small `widget::paint` combinator
composes the existing measured leaf and render continuation; swatches,
delimiters, and Fidget image leaves use it too. Fidget still renders in
projection, at the same resolution and with the same camera inputs; only its
one-image drawing wrapper is gone. Explicitly stored drawing descriptions
retain their interpreter.

The new open-RGBA-picker canary exercises these controls independently of the
large documents. Feature-free release binaries were alternated in
before/after/after/before/before/after order, with five warm-up and 3,000 measured
frames each, at 600 × 400 logical pixels and scale 2. The isolated color change
gave:

| Metric | Before | After |
| --- | --- | --- |
| Whole-frame medians across three runs | 15.58 / 15.62 / 15.58 µs | 12.46 / 11.67 / 11.46 µs |
| Preparation, median | 9.04–9.17 µs | 5.42–5.88 µs |
| Paint + handler disposal, median | 2.79 µs | 2.46–2.67 µs |
| All allocations per picker frame | 254 | 241 |

This is roughly 20–25% of this very small frame: only 3–4 µs saved. It is not a
claim of a noticeable whole-editor improvement. IoP source remains at 87,941
allocations per frame. All nine feature-free canaries also passed in an
interleaved before/after/after/before run with 60 measured frames each; source,
picture, and Fidget CPU-fallback results showed no consistent timing change.
The image-wrapper cleanup is not expected to materially accelerate Fidget.
A further six-run source check with 300 measured frames gave before medians
3.67 / 3.81 / 4.03 ms and after medians 4.11 / 4.13 / 3.71 ms. The changing
costs across unchanged phases make this noisy, not evidence of a source
speedup; small effects remain unresolved.

A temporary recording comparison confirmed identical geometry, brushes,
gradients, transforms, and command order for all three picker controls, three
colors, and two display scales. The temporary test was removed; permanent
tests check marker and fill geometry at several positions and scales, swatch
metrics and its inset border, raster-image transforms, and paint deferral and
clipping. Workspace tests and native all-target checks pass. The web check
passes with its existing unused `drawn_menu` and `Quit` warnings. No cache,
partial invalidation, event-policy change, or runtime profiling was added.

## Compose value decorations before wrapping layout — 2026-09-07

Baseline: `2a5b751`. Temporary finer allocation scopes split projection
construction from its editor adaptation. An IoP source frame attributed about
25,100 allocations to partial/fallback construction, 7,100 to its value-level
wrappers, 3,180 to materializing widget sites, 2,736 to projection targets, and
1,410 to appending descendant paths. Copying the cycle-detection ancestry was
only 155 allocations. These are exclusive diagnostic scopes, not additive
wall-time speedup estimates; the temporary scopes were removed afterward.

`prepare_value` separately wrapped the same child for its navigation/highlight,
optional reference background, and pick backstop. All three preserve its extent.
It now applies those same functions, in the same order, inside one
`ChoiceLayout::map`. This removes intermediate choice-graph wrappers without
changing the callbacks, placement outputs, input order, or alternative policy.
There is no new layout operation or special-case search optimization.

Feature-free release binaries were alternated in before/after/after/before/
before/after order, with five warm-up and 300 measured frames each, at
1400 × 900 logical pixels and scale 1:

| Metric | Before | After |
| --- | --- | --- |
| Whole-source-frame medians across three runs | 3.70 / 3.69 / 3.70 ms | 3.61 / 3.65 / 3.63 ms |
| Choices + settled geometry, median | 279–283 µs | 238–246 µs |
| Value-wrapper allocations per frame | 7,096 | 4,496 |

The whole-frame gain is about 2%, with 2,600 fewer allocations. The reduced
choice time is from visiting fewer geometry-preserving wrappers, not from
searching less accurately or selecting different alternatives. Actual placement,
hover, and paint retain the existing continuations. No cache or partial
invalidation was introduced.

All nine canaries passed in an additional before/after/after/before run with
60 measured frames. IoP picture medians were 22.40 / 22.39 ms before and
22.51 / 22.70 ms after; this pass does not improve drawing-program execution.
Fidget CPU-fallback results remained comparable across the alternating runs.
Workspace tests and native all-target checks pass; the browser check retains
its two existing unused-code warnings. Fifteen of the 19 SVG fixtures are
byte-identical. The four sample variants differ only in the dim glyph paths
for freshly minted short cell IDs; surrounding geometry and paint order match.

## Shared blobs and narrower widget capabilities — 2026-09-08

Baseline: `235a5a8`. The storage experiment compared `Vec<u8>`, `Arc<[u8]>`,
`Arc<Vec<u8>>`, and a test-only enum that shares above 64 bytes. Representative
medians from seven interleaved rounds on the same ARM64 Mac:

| Operation | Owned vector | Shared vector |
| --- | ---: | ---: |
| Construct/dispose 8 bytes | 13.4 ns | 28.8 ns |
| Clone/dispose 8 bytes | 13.0 ns | 3.5 ns |
| Construct 8 bytes, clone/dispose four times | 67.0 ns | 37.2 ns |
| Construct/dispose 4 MiB | 49.6 µs | 49.6 µs |
| Clone/dispose 4 MiB | 49.8 µs | about 6 ns |
| Clone, edit, dispose a shared 4 MiB value | 50.0 µs | 50.2 µs |

These are hot microbenchmarks, not frame-speedup predictions. Construction
includes copying the input fixture into an owned vector; adopting that vector
into `Arc<Vec<u8>>` adds only its control allocation. Converting it into
`Arc<[u8]>` copies the payload again: the 4 MiB construction case measured
111.8 µs. The threshold variant preserves small-vector construction cost but
also its clone cost; there was no need to carry that second representation into
production to retain the frame performance below.

`gid::Blob` now wraps `Arc<Vec<u8>>`. Reads remain byte slices, `Value::from`
still accepts vectors, and `make_mut` performs ordinary copy-on-write. There
is no cutoff, byte interner, cache, or new serialization convention. Tests check
adoption without copying, shared clones, unique/shared edits, content equality,
hashing, and the existing GID serialization and text-bridge round trips.

The separate widget change splits the former combined `Site`: selection and
picking request only their target/value/callback; line controls request a
`LineSite` whose `LineInput` is absent at read-only locations. The host no longer
builds line-edit callbacks for selectable delimiters, or editing/selection
callbacks for read-only lines. Writable line behavior, default caret placement,
and library-value selection are unchanged. Tests make line-capability access
panic for delimiter controls, and verify read-only lines emit no input handlers.

Saved feature-free release binaries were run in baseline / blobs / both / both /
blobs / baseline order, five warm-up and 180 measured frames per workload:

| Whole-frame medians | Baseline | Shared blobs | Plus narrower capabilities |
| --- | --- | --- | --- |
| IoP source, 1400 × 900 @1 | 3.93 / 3.67 ms | 3.63 / 3.65 ms | 3.60 / 3.59 ms |
| IoP picture, 500 × 500 @1 | 23.22 / 22.92 ms | 23.07 / 23.09 ms | 23.14 / 23.35 ms |

The first baseline source run was noisier (4.45 ms p95); do not attribute that
whole difference to blobs. The source stays in its previous range, with a small
improvement from narrower capabilities; the picture shows no meaningful change.
An earlier baseline/blob/blob/baseline run passed all nine canaries, including
the five Fidget CPU-fallback workloads. No native GPU/presentation claim follows
from these headless results.

The final combined build also passed all nine canaries, the full workspace test
suite, and the native all-target check. The browser check passes with its existing
unused `drawn_menu` and `Quit` warnings. No runtime profiling or new dependencies
were added; storage alternatives live only in the opt-in test.

## Direct editor widget handlers — 2026-09-08

Progred's display and library modules now live in the application crate.
Document-aware widgets borrow their current inputs while preparing, then their
handlers receive `&mut Editor` and call ordinary editing helpers. This removes
the projection `Hooks` dictionary, the narrower `Site`/`LineSite` callback
factories, and the drawn menu's hook bundle. Puri and the measured box algebra
remain independent; this introduces no cache or action-dispatch layer.

Feature-free release test binaries from `df8c6bf` and the refactor were run
alternately under the same Seatbelt policy, with five warm-up and 90 measured
frames per workload:

| Whole-frame medians | Before (two runs) | After (two runs) |
| --- | --- | --- |
| IoP source, 1400 × 900 @1 | 4.01 / 3.92 ms | 3.90 / 3.91 ms |
| IoP picture, 500 × 500 @1 | 24.19 / 24.19 ms | 24.34 / 24.78 ms |

Treat this as effectively neutral performance, not an optimization win. The
picture's second after run was noisier (46.39 ms maximum); its paint/evaluator
work remains overwhelmingly dominant. These are headless projection/recording
checks, not measurements of native GPU presentation.

After the final gesture-adapter and test cleanup, the same 90-frame checks
measured 3.59 ms for source and 23.30 ms for picture. The full workspace suite
passed 629 tests; native and browser builds still check successfully.

## Native continuations and widget-owned scrubbing — 2026-09-08

Native completion offers now select through staged Rust continuations rather
than constructing and evaluating Grap functions. Completion insertion and
gesture edit runs no longer carry callback dictionaries. Precision-aware scrub
spelling belongs to the number widget's selected line editor, not the frame
pipeline. The test-only line-description observer was removed; tests use pure
descriptions/conversions or actual editor handlers instead.

The same feature-free release checks (five warm-up, 90 measured frames) measured
3.57 ms median / 3.71 ms p95 for IoP source at 1400 × 900 @1, and 23.48 ms median /
24.16 ms p95 for its picture at 500 × 500 @1. Compared with the preceding
3.59 / 23.30 ms check, this is neutral at the precision of these separate runs,
not evidence of a speedup. These remain headless checks, not GPU presentation
measurements. All 632 workspace tests pass. Native checks are clean; the web
check retains its existing `drawn_menu` and `Quit` warnings.

## Streaming hover and explicit after-hover binding — 2026-09-08

Settled placements now run ordinary hover probes in painting order. Only
floating placements are queued. The retained hover-callback sequence and its
paint/navigation reversal bookkeeping are gone. Puri's generic `AfterHover`
collection binds the final hover before producing independent paint and
handlers; paint callbacks retain that frame's input rather than reading live
editor hover during presentation. No cross-frame memo or native render enum
was added.

Feature-free release checks used five warm-up and 90 measured frames:

| Whole-frame median | Before | After, two runs |
| --- | --- | --- |
| IoP source, 1400 × 900 @1 | 3.71 ms | 3.39 / 3.50 ms |
| IoP picture, 500 × 500 @1 | 23.63 ms | 23.82 / 23.90 ms |

This suggests a modest source improvement, with effectively neutral picture
cost. These are headless checks, not GPU presentation measurements. Phase
timings now label placement and hover together, followed by after-hover
binding, so their individual columns are not directly comparable to old runs.
On the final source run those phases took 0.716 and 0.149 ms respectively;
the old placement plus hover/handler phases took 0.414 plus 0.700 ms.

The full workspace suite passed 634 tests, followed by an additional passing
nested-floater ordering regression. Browser checks retain only the existing
`drawn_menu` and `Quit` warnings.

## Hover attribution: shared versus derived at use — 2026-09-08

The preceding source canary had no pointer, so it did not measure the benefit
of sharing the hovered source's attribution. A temporary headless experiment
exercised `Editor::build_frame` and its actual paint continuations at 1400 × 900
@1, with real visible source hover targets in the IoP document. Source-only
removed the pane declaration; the other case kept the source and picture panes.

Four variants rotated order each iteration: both descriptors shared, secondary
identity derived at each use, canvas source trace derived at each use, and both
derived at use. At-use variants skipped the corresponding eager derivation.
Temporary test-only accessors counted actual consumers and used an immutable
document/library snapshot established outside timing. The same accessor/counting
overhead applied to shared variants. Render commands were compared outside
timing for every variant/target combination. The initial run used 90 samples;
the expanded repeat used five warm-up and 120 measured samples per variant.
All experimental hooks and the temporary test were removed afterward.

| Source-only build + paint median | Shared | Secondary at each use |
| --- | --- | --- |
| Deep value hover (21 path steps) | 3.415 ms | 3.570 ms |
| Shallow value hover (4 steps) | 3.407 ms | 3.441 ms |
| No hover | 3.433 ms | 3.426 ms |

These times include handler disposal, but not final draw-list disposal or GPU
presentation. The deep-hover result repeated the initial 3.421 → 3.571 ms result.
There were 439 secondary-identity consumers with a value hovered (440 with no
hover). Isolated derivation took roughly 0.33 µs for the deep path and 0.05 µs for
the shallow path: cheap individually, but repeated enough to add about 4.5% or
1% respectively. No-hover differences were noise.

With both panes, 301 secondary consumers and one canvas-trace consumer ran;
total build + paint was about 26 ms, dominated by drawing evaluation. Moving
trace derivation to its consumer had no stable measurable impact across runs.
One trace derivation took about 0.18 µs for the deep path and 0.04 µs for the
shallow path. Unlike secondary identity, trace derivation can allocate an owned
cell-relative path slice, but it does not repeat per drawing command.

Keep shared secondary attribution: it avoids hundreds of redundant graph walks
without a cross-frame cache. Eager canvas-trace derivation has no demonstrated
performance advantage in this workload; its placement is an API/design choice,
not a measured optimization requirement.

## Pointer hover over retained frame geometry — 2026-09-13

A temporary headless comparison used the complete IoP document at 1400 × 900
@1, alternating between two drawing-source targets with source linking enabled.
Both variants rebuilt all content. The old ordering built to discover hover,
dispatched its reaction, then rebuilt; the new ordering queried the installed
frame's probes, dispatched the reaction, then built once. Variant order alternated
each iteration, with five warm-ups and 60 measured transitions per variant.

| Motion through completed headless painting | Median | p95 |
| --- | ---: | ---: |
| Fresh build before hover reaction | 55.94 ms | 60.28 ms |
| Retained probes before hover reaction | 28.73 ms | 30.91 ms |
| Retained hit-test alone (1,000 queries) | 6.75 µs | 7.00 µs |

The variants produced identical hover targets, source scroll offsets, and draw
commands after normalizing only per-context font allocation identities. These
times include building and headless painting, not native GPU/display latency.
The temporary comparison was removed; permanent tests cover single-build pointer
reactions, paint-gated follow-ups, probe ordering/clipping/view ownership, and
drawing probes reusing the installed recording without reevaluating Grap.

Separate 90-frame canaries measured source at 3.38 ms before and 3.50 ms after,
and picture at 24.04 ms before and 23.60 ms after. Retaining source probes has a
small construction/storage cost; these separate runs do not establish its exact
size or a picture speedup. No successor frame reuses the previous drawing's
evaluation: retained geometry belongs only to the installed frame's input handling.
