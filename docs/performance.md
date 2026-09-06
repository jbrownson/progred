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
- Projection/measurement time and the remaining frame work. Library work occurs
  in whichever phase normally invokes it: Fidget renders during projection,
  while the IoP canvas program runs during placement.

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

The additional filters are `fidget_torus_profile_loop`,
`fidget_tanglecube_profile_loop`, and `fidget_gyroid_profile_loop`. These use the
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
