# Frame performance checks

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
| RGBA picker | 600 × 400 | 2 | Project a selected color with its picker open |
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
