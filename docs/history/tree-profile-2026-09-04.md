# Tree demo CPU profile — 2026-09-04

Baseline: `b6f93e191f747d2cf26907bca613e3515e6eb935`, clean working tree.
Measured on the local ARM64 Mac, optimized `release` builds under the repository's
Seatbelt wrapper. The app was not launched.

## Result

One small change was worth keeping: materialize `match`'s mismatch absents only
when all cases fail. The canvas benchmark's median fell from **25.3 ms to
24.4 ms**, about 4%. A second candidate run also measured 24.4 ms. A repeated
baseline was slower, at 26.6 ms, so the exact percentage remains subject to
machine noise. This is a modest saving, not enough to bring this CPU work alone
under a 16.7 ms frame budget.

No cross-frame memo, partial invalidation, dependency change, or drawing-policy
change was introduced.

## Method and limits

`projection::tests::frame::profile::iop_tree_profile_loop` projects the declared picture pane,
places it, evaluates Grap, records its drawing commands and picking geometry,
and replays into the headless `DrawList` canvas. Each iteration rebuilds the
picture. Measurements below exclude the first five iterations. The reported
frame timer excludes fixture setup and destruction of the returned draw list.

The harness now retains the library stack, font context, layout context, and
approved text-shaping cache between iterations, as the app does. Previously it
recreated them every iteration: font discovery occupied much of the sampler's
output despite being outside the reported frame timer. This correction improves
profiling fidelity; it is not an application speedup.

The document-view canary now clips to its top 1400 × 900 viewport. Without a
viewport, it also rendered the picture nested under the document's `panes`
field, giving a misleading 30.6 ms “source” measurement. With clipping and warm
resources it measured **5.4 ms**, including **4.0 ms** of projection. This is a
separate view benchmark, not a measurement of the app's complete window.

There is no GPU rendering, display synchronization, window presentation, or
interactive event processing in these measurements. Hover and selection
highlighting were not separately timed. Actual frame rate still needs the
user's visual testing.

## Experiments

| Experiment | Measured frames | Median canvas CPU time | Disposition |
| --- | ---: | ---: | --- |
| Warm baseline | 895 | 25.3 ms | Reference |
| Stage arithmetic argument lookups | 95 | 25.9 ms | Reverted |
| Retain a number's original `Record` directly | 115 | 25.3 ms | Reverted |
| Above plus inline cross-crate `Context::field` | 115 | 25.5 ms | Reverted |
| Original `Record` plus shared immutable closures; runtime values 56 → 40 bytes | 115 | 25.9 ms | Reverted |
| Construct mismatch absents only after every case fails | 155 | 24.4 ms | Kept |
| Baseline repeated | 155 | 26.6 ms | Reference |
| Deferred mismatch construction repeated | 155 | 24.4 ms | Kept |

The arithmetic experiment used the existing staged-function facility. Its
lookup savings did not outweigh the other costs. The compact representation
added closure allocations; smaller values alone did not improve this workload.
Neither result justifies adding the machinery.

## What the sample showed

A ten-second, 1 ms interval macOS CPU sample contained 4,366 samples under
`drawing::record_program`. Within that subtree, approximate exclusive shares
included field lookup (8%), compiled-call dispatch (5.5%), environment updates
(5.2%), memory copying (4.8%), numeric evaluation (4.7%), and runtime-value cloning
(4.2%). Other time was spread across definition dispatch, foreign-function
dispatch, binding lookup, allocation, and drops. These are sampled CPU shares,
not separately timed operations.

Recording/evaluation accounted for about 97% of the sampled headless settlement
path. The native reference emitted the same drawing in about 0.59 ms, but it
bypasses the evaluator and source attribution; it is not an equivalent editor
implementation or a promised attainable speedup.

The remaining cost is distributed through interpreter execution. No large,
straightforward missed optimization was demonstrated in this round. A larger
evaluation or representation change should earn its complexity against this
benchmark and the semantic conformance tests before adoption.

## Retained change and validation

In `libraries/src/control.rs` (`match_prepare`, line 163, and `select`, line 387),
both prepared and referenced match cases used to
construct a GID absence record after each mismatch, even if a later case
succeeded. They now try cases without building that failure list. If every case
fails, a second traversal constructs the same ordered causes. It does not repeat
matching or evaluation. This trades an extra traversal on total failure for no
discarded absence records on success. Malformed cases, invalid binders, selected
absent results, fuel exhaustion, and early success still return at the same point.

The existing tests cover ordered causes, a subject evaluated once, exact fuel,
referenced and inline patterns, and lazy binder errors. The real tree's complete
SVG command serialization still exactly matches the native reference: 511
branches and 7,680 blossoms. All 410 workspace tests pass (two profiling tests
ignored), as does the browser compilation check; the browser retains its two
existing unused-code warnings.

Suggested addition to `AGENTS.md`'s testing guidance: steady-state profiling
loops should reuse app-lifetime resources and apply a representative viewport;
report setup and unbounded-document costs separately.

Reproduce the canvas run:

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  --config 'env.IOP_PROFILE_ITERATIONS="160"' \
  iop_tree_profile_loop -- --ignored --nocapture
./tools/sandbox-cargo test --release -p progred --lib \
  iop_tree_source_profile_loop -- --ignored --nocapture
./tools/sandbox-cargo test --release -p progred --lib \
  iop_tree_projects_through_grap_into_puri_ink -- --nocapture
```

Local evidence is under `target/sandbox/tmp/tree-profile-*.log`, with the initial
stack sample in `tree-profile-baseline.sample.txt`. Those generated files are
not committed.


## Follow-up: replacing transactional calls with effect checks

The later `a174dcb` change added snapshots around function calls and used
persistent vectors for drawing so declined calls could discard their effects.
A fresh comparison on the same local Mac used 100 iterations of the headless
picture benchmark, with release builds under Seatbelt and no app launch:

| Implementation | Whole-loop average per frame |
| --- | ---: |
| `a174dcb`: per-call snapshots and persistent drawing collections | 29.9 ms |
| Per-call effect-counter checks and local drawing vectors | 24.3 ms |

This is about 19% less time in this comparison. These are whole-loop averages,
including initial iterations and output destruction, unlike the warmed frame
medians in the original experiment. The comparison does not isolate the counter
checks from the collection change, and it does not measure interactive frame
rate. Its result is near the earlier implementation's timings, without proving
that the new check has zero overhead.

The replacement changes the contract: a function must decline before effects.
Rust capabilities increment the context's effect counter when writing, and a
call that subsequently declines halts the evaluation with a tagged error and
prints to stderr. Effects in arguments and nested calls count too. Ordinary
absents still retain their effects. The host can discard the complete temporary
editor operation or drawing recording; individual calls never roll back state.

Wrapping the effect body in `context.effect(|| operation)` subsequently measured
25.2 ms/frame and 24.0 ms/frame in two 100-frame runs of the same workload.
These runs show timing variability rather than establishing any helper overhead.

To reproduce the comparison's workload:

```sh
./tools/sandbox-cargo test --release -p progred \
  --config 'env.IOP_PROFILE_ITERATIONS="100"' \
  iop_tree_profile_loop -- --ignored --nocapture
```
