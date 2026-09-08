# Frame performance checks

The opt-in tests in `progred/src/projection/tests/frame/profile.rs` share one
headless frame harness. They are not assertions about interactive frame rate,
and ordinary builds and launches contain none of this instrumentation.

Run the current canaries serially in an optimized build:

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  profile_loop -- --ignored --nocapture --test-threads=1
```

Use `fidget_orbit_profile_loop`, `iop_tree_profile_loop`, or
`iop_tree_source_profile_loop` as the filter to isolate a workload. The default
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

`BenchFrame` takes a document, source-qualified root path, selection,
annotations, available width, placement origin, clipping rectangle, and pointer.
`BenchContext` retains the library stack, fonts, layout context, and text shaping
cache. A `ProfileView` configures logical size and display scale, using zero
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
  resolution plus settled geometry, placement, hover/handler construction,
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
| Fidget orbit | 400 × 600 | 2 | Advance yaw by 2° per frame; pitch 60°, zoom 1 |
| Torus orbit | 400 × 600 | 2 | Same camera sequence |
| Tanglecube orbit | 400 × 600 | 2 | Same camera sequence |
| Gyroid sphere orbit | 400 × 600 | 2 | Same camera sequence |
| Fidget cube orbit | 400 × 600 | 2 | Same camera sequence; Rhino-derived quadratic faces |

The additional filters are `fidget_torus_profile_loop`,
`fidget_tanglecube_profile_loop`, `fidget_gyroid_profile_loop`, and
`fidget_cube_profile_loop`. These use the
same helper as the original Fidget canary, changing only the document. Their
editable formulas and sources are described in [the examples guide](../examples/README.md).

Fidget receives ordinary camera annotations at the viewport's source path. It
uses the library's normal automatic backend: GPU when available, CPU fallback
otherwise. Its 800 × 1200 raster is produced through the real preview projection,
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
