# GPU spill experiment — 2026-09-16

This is a headless experiment, **not an app renderer change**. Command+9 still
uses the CPU implicit renderer. All tests below used the current Apple Silicon
Mac's Metal backend; no editor/window was launched.

**Status: paused after four passes.** Keep the CPU app renderer. Spill support
works in the tested scenes, but neither larger program storage nor independent
screen-region batches make this GPU interpreter competitive with the CPU JIT.
The code, reproducible diagnostics, and measurements are retained as a checkpoint,
not an active rollout. Possible future directions include shared-parent GPU
traversal or CPU specialization followed by GPU evaluation; neither is implemented
or demonstrated to improve end-to-end performance.

## What changed

The pinned Fidget GPU bytecode already encodes register loads and stores, but
the shader interpreter and simplifier stopped at `OP_MEM`. The local `fidget-wgpu`
experiment implements those instructions. Its first version used private arrays;
voxel evaluation now uses bounded external scratch, as described below. Backward
simplification tracks spill-slot liveness separately from register liveness:
a store defines a memory slot, not the reserved register 255.

Shader pipelines are now specialized by register and spill storage requirements.
The same implementation is used by interval, scalar, gradient, and color
evaluation. The experimental choice-history capacity is explicit on the render
shape; its default is upstream's 32 words (512 two-bit choices).

Spill execution requires the non-default `experimental-spills` feature. Progred's
`gpu-experiment` feature enables it for the ignored diagnostic. **Do not use that
feature for an ordinary interactive build**: large shaders can fail pipeline
creation, and correctness/performance are not established for arbitrary scenes.
Without the feature, spilling geometry and color tapes return an error before
submission instead of silently producing incomplete GPU output. Progred's
existing standalone preview fallback can then use the CPU. CAM already chooses
the CPU directly.

## First pass: private spill arrays

The small regression scene deliberately lowers to only three registers to force
spills. Its GPU image is bit-identical to the nonspilling GPU image. It also
matches CPU coverage, sample depth, and normals after accounting for an existing
encoding difference: CPU depth stores sample Z + 1; GPU stores sample Z.

The current CAM fixture records 6,588 segments. At playback 0.5, its stock has:

- 339,008 VM instructions, including 84,910 loads/stores;
- 12,747 spill slots beyond the 255 registers;
- 64,689 min/max/other choice operations.

Stock-only timings (seconds, not a statistical benchmark):

| Image / depth samples | Prepared CPU | GPU first run | GPU repeated run |
|---|---:|---:|---:|
| 32 × 72 / 128 | about 1.6–1.7 | 3.16 | 2.17 |
| 85 × 192 / 192 | 1.59 | 4.84 | 4.49 |
| 85 × 192 / 768 (4× depth, exact spill allocation) | 2.72 | 12.24 | 11.27 |

The larger comparison explicitly retained CPU root compilation using the same
prepared-scene mechanism as the app; root preparation was 13 ms. GPU repeated
runs retained pipelines and work buffers. Timings exclude Grap/toolpath and tree
construction, final lighting/composition, and app dispatch. GPU timings include
submission and geometry readback. Each successful comparison had zero coverage,
depth, or normal mismatches (normal tolerance 1e-4).

The first two rows rounded private spill storage to 12,800 slots; the retained
implementation and final row use the exact required count. Rounding to the next power of two
(16,384 slots) failed Metal gradient-pipeline compilation: `Compute function
exceeds available stack space`. A gradient is four floats, so that attempted
array alone was 256 KiB per invocation. At playback 1.0 the stock requires 18,655
spill slots and 463,708 instructions; even tightly sized storage exceeds the
stack limit. Reducing image resolution cannot reduce this per-invocation cost.

## Other first-pass experiments

- A depth-first SSA scheduling candidate did not reduce the selected stock tape's
  spill requirement. It was removed rather than adding ineffective compilation.
- Increasing the history from 512 choices to 4,096 or 16,384 choices still matched
  the small CAM image, but repeated GPU times were 2.24 and 2.48 seconds: no win.
- Retaining 65,536 choices produced an incorrect empty image. That is a failed
  experiment, **not a speedup**. The cause is not established; it could be in our
  spill-aware simplification, existing specialization, or GPU resource behavior.
  Keep the configurable history only as an opt-in diagnostic to reproduce it.

The existing 64 MiB tape workspace does **not** include private shader storage,
external scratch, driver/compiler allocations, or total process/GPU memory.

## Second pass: bounded external voxel scratch

Voxel evaluation now reserves at most 64 MiB for variables plus scratch, bounded
also by the device's storage-binding limit. There are at most 4,096 evaluation
lanes, rounded down to a whole 64-thread workgroup. Each lane owns its scratch
until its invocation finishes. Root, interval, voxel, and normal passes dispatch
within that lane count and stride over any remaining work. A tape too large for
even one workgroup returns a specific scratch-capacity error before submission.
Nonspilling tapes retain their original dispatch. This is not a per-pixel buffer.

Scratch shares the existing variables binding: the input prefix remains
read-only by convention; only the scratch tail is written. This avoids exceeding
the interval shader's eight-storage-buffer limit. Scalar/interval/gradient
passes transfer one/two/four floats per spill respectively; the allocation is
sized for gradients and reused by the other passes. Components are interleaved
across lanes for contiguous access by neighboring threads.

The simplifier's private spill-liveness array is now a bitset rather than an array
of booleans. For the full stock, that reduces about 73 KiB of per-invocation
liveness storage to about 2.3 KiB. Pixel and color evaluation still use private
spill arrays and retain their earlier size limitation.

The fully cut stock now compiles and renders correctly with 18,655 spill slots.
The formerly blank halfway-stock test with 4,096 choice words also matches the
CPU, as does full stock with 8,192 words (enough for all 89,885 choices). The exact
cause of the old blank result is **not isolated**: scratch representation,
liveness storage, and dispatch concurrency changed together. Passing these
tests is not evidence of general correctness for every expression/device.

Small stock-only comparison, 32 × 72 pixels / 128 depth samples:

| Playback / choice words | Prepared CPU | Warm GPU | GPU workspace |
|---|---:|---:|---:|
| 1.0 / 32 | 3.21 s | 23.07 s | 118 MiB |
| 0.5 / 4,096 | 1.79 s | 9.04 s | 126 MiB |
| 1.0 / 8,192 | 2.37 s | 22.06 s | 118 MiB |
| 1.0 / 8,192, omit unused spill components | 2.37 s | 21.47 s | 118 MiB |

Every comparison had zero coverage, depth, or normal differences. These are
single runs, not statistical benchmarks; some builds overlapped GPU execution.
The final traffic cleanup does not establish a significant speedup. Full stock
gets 192 scratch lanes (~55 MiB), halfway stock 320 (~62 MiB), plus the existing
64 MiB tape arena and small geometry buffers. Workspace totals exclude private
shader and driver memory. The final implementation retains the traffic cleanup.

The expanded GPU regression also reuses a workspace across free-variable changes,
nonspilling/spilling transitions, and larger choice histories. Image sizes extend
beyond the lane count so strided reuse is exercised. Pure tests check scratch
budget boundaries; shader-layout/validation tests cover both storage variants.

## Second-pass conclusion

Implementing `OP_MEM` and bounding scratch fixes the tested correctness and stack
failures, but **does not accelerate this CAM workload**. Do not switch the app.
The next useful experiment would measure time per GPU stage and how much of each
specialized tape survives, rather than further guessing storage/history sizes.
Limited lane concurrency and spill traffic are plausible costs; the third pass
below establishes an additional program-arena bottleneck. Merely widening the register index or increasing array limits does
not address this workload. No GPU execution timeout,
progress/cancellation integration, or app backend switch was added.

## Third pass: GPU stage timing and program-arena exhaustion

The opt-in `voxel::diagnostics` helper measures actual GPU timestamps at compute
pass boundaries and snapshots the program arena after the first stratum's
interval work. It calls the existing stage functions with the same preparation,
but splits their compute passes to attach timestamps. That can affect scheduling;
the comparison therefore first runs the ordinary GPU path and checks both
against the CPU. Snapshot readback adds temporary diagnostic memory outside the
reported workspace (one arena-sized GPU read buffer and a host copy).

Full stock, 32 × 72 / 128 depth, complete 8,192-word choice history, 64 MiB arena:

| Stage | GPU time |
|---|---:|
| Root intervals | 0.410 s |
| Subtile intervals and sorting | 2.816 s |
| Voxel samples | 16.188 s |
| Normals | 2.453 s |
| Repacking, merging, clearing | less than 0.001 s |

Instrumented wall time was 22.04 s, versus 22.46 s for the ordinary run. All
coverage/depth/normal comparisons passed. Most time is genuine GPU evaluation,
not host submission or readback.

The snapshot contained 8,388,574 allocated tape words out of 8,388,608: only
272 bytes remained in the 64 MiB arena. The root program has 463,708 instructions;
among nonzero 4³-tile program-map entries, the median was still 436,331
instructions and 118,022 spill operations. Those 314 entries pointed to only
19 distinct starts. When allocation fails, `simplify_tape` returns zero and the
child retains its parent program. A failed attempt also leaves previously
allocated chunks until the stratum reset. Thus finishing the render is not
evidence that spatial specialization completed successfully.

A controlled diagnostic doubled only this arena to 128 MiB (the default device
binding limit, including its header). It too filled almost completely, but 100
distinct fine-tile starts survived and voxel time fell to 9.35 s. Total profiled
wall time was 15.79 s versus the ordinary 64 MiB run's 22.86 s; CPU was 2.52 s.
Geometry still matched exactly. Workspace grew from 118 to 182 MiB. This is a
useful causal test, **not a changed default or a recommended memory setting**.

One small change is retained: voxel rendering skips work wholly outside the
actual image's X/Y bounds, and interval domains clamp their upper X/Y ends to
those bounds. Allocation still uses the padded 64-voxel dimensions. At the
original 64 MiB budget the snapshot's root program became smaller, but the
arena still filled and total time was 21.38 s. This is not a meaningful speedup
claim from a single run; it removes unnecessary work without increasing memory.

The next focused experiment was bounded spatial batches: specialize a region,
evaluate its pixels and normals while those programs are available, then reuse
the arena. This directly addresses the measured lifetime/working-set problem.
The fourth pass below tests the simplest version. Simply increasing choice
history or widening registers does not make room for the resulting specialized
programs.

## Fourth pass: bounded screen regions

The headless diagnostic can now render disjoint screen regions serially into
one image. It reuses the same GPU workspace and 64 MiB program arena, resetting
the arena for each region. The cropped camera preserves the original sampling
density and all depth samples; this is not a lower-resolution approximation.
It uses the existing voxel renderer without new shader behavior or app policy.
The small camera matrices can differ in floating-point rounding, so this is
mathematical sampling equivalence, not a promise of bitwise equality for every
possible surface on a sample boundary.

Full stock, 32 × 72 / 128 depth, 8,192-word choice history:

| Method | Wall time | Same-run CPU reference |
|---|---:|---:|
| Whole-frame GPU, before 16-pixel batches | 22.42 s | 2.46 s |
| 16 × 16 screen regions | 25.86 s | 2.46 s |
| Whole-frame GPU, before 8-pixel batches | 22.57 s | 2.51 s |
| 8 × 8 screen regions | 44.30 s | 2.51 s |

Every comparison passed: identical coverage and sampled depths, and normals
within the existing 1e-4 tolerance. These are individual diagnostic runs, not
statistical performance claims. Repeated program upload, submission, and
readback are included. Because the baseline and batches share the workspace,
its reported high-water capacity remains 118 MiB; batching does not shrink
previously allocated buffers.

Profiling the expensive region at screen (0, 32), size 8 × 8, showed why:

| Stage | GPU time |
|---|---:|
| Root interval evaluation and specialization | 0.440 s |
| Subtile interval evaluation, specialization, and sorting | 1.410 s |
| Voxel samples | 0.245 s |
| Normals | 0.112 s |

The profiled wall time was 2.26 s; the ordinary render of that region took
2.35 s, and CPU took 0.134 s. About 82% of GPU stage time is now interval work,
rather than voxel sampling. The first-stratum snapshot used 4,007,966 of
8,388,608 tape words (about 31 MiB, below half the arena). All 20 nonzero
fine-tile program entries had distinct starts, with instruction counts
32,332 / 97,701 / 170,983 (min / median / max), versus the whole-frame median
436,331. Thus the local memory pressure is relieved and useful pruning occurs,
but repeatedly doing this work independently for each crop loses overall.
This snapshot is evidence for that region, not a proof that every region and
stratum stays below capacity.

The simple batching path is **not an app optimization**. A deeper experiment
would retain shared ancestor programs and recycle storage only for bounded
groups of descendants, keeping all evaluation and normal lookup within those
programs' lifetimes. That would require changing the GPU traversal, not just
cropping the image. These measurements do not establish that such a redesign
would beat the CPU JIT, and no such change is implemented here.

## Reproduction

Builds stay inside the repository's dependency sandbox:

```sh
./tools/sandbox-cargo test --release -p fidget-wgpu --lib compile_
./tools/sandbox-cargo test --release -p fidget-wgpu --lib spills_require_explicit_opt_in
./tools/sandbox-cargo test --release -p fidget-wgpu --lib \
  --features experimental-spills --no-run
./tools/sandbox-cargo test --release -p progred --lib \
  --features gpu-experiment --no-run
```

The already-built test executables need explicit Metal access outside that
build sandbox. Use the executable paths printed by the build, with a bounded
external timeout; do not launch the application. Tests:

- `spill_tests:: --include-ignored --nocapture --test-threads=1`
- `cam_render_profile --ignored --nocapture --test-threads=1`, with
  `CAM_GPU=1 CAM_GPU_STOCK=1 CAM_PROGRESS=0.5 CAM_HEIGHT=72`.

`CAM_GPU_DEPTH` defaults to 1; the app's final depth refinement can be requested
with 4. `CAM_GPU_CHOICE_WORDS` defaults to 32. Playback 1.0 and choice history
4096 failed with private arrays but now pass the small voxel comparison; 8192
also passes at playback 1.0. Start small:
already-submitted GPU work does not acquire cooperative cancellation merely
because the host test has a timeout.

`CAM_GPU_PROFILE=1` replaces the second GPU run with the instrumented stages and
first-stratum tape snapshot; it requires timestamp-query support. Optional
`CAM_GPU_TAPE_MIB=128` changes only that diagnostic run's arena. The default
unprofiled renderer still allocates 64 MiB. A complete profiling invocation adds
`CAM_GPU_PROFILE=1 CAM_GPU_CHOICE_WORDS=8192 CAM_PROGRESS=1 CAM_HEIGHT=72` to the
stock-only comparison. Counters describe the captured first stratum, not a
time-weighted count of all executed instructions.

`CAM_GPU_BATCH=16` replaces the second run with bounded screen regions;
`CAM_GPU_BATCH=8` tests finer batching. `CAM_GPU_REGION=0,32,8` restricts both
the CPU reference and GPU runs to one region of the original camera, useful
with `CAM_GPU_PROFILE=1`. Batch mode takes precedence over profiling on the
second run; use region mode, without batch mode, to profile one crop. All are
headless diagnostic options, not app configuration.

Third-pass verification: all 47 vendored GPU tests passed on Metal,
including two native spill regressions, shader/layout checks, the scratch-budget
check, the tape-inspection unit test, and existing pixel/color/voxel rendering tests. All 737 normal Progred
tests passed (32 ignored). The normal-build geometry/color spill-rejection test
passed separately. The regenerated GPU patch applies cleanly to the pinned
upstream source. No app was launched and no changes were committed in this pass.

Fourth-pass verification: all 49 vendored tests passed on Metal, including the
new crop-transform check and spilling/nonspilling batch comparisons with uneven
edge regions. Both full-stock batch runs and the focused region profile matched
their CPU references. The GPU patch was regenerated and checked against the
pinned upstream tree. This pass changes only opt-in diagnostics/tests and their
documentation; the app still uses the CPU path. No app launch or commit.
