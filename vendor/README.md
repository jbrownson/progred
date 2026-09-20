# Local Fidget patches

`fidget-core`, `fidget-jit`, `fidget-raster`, `fidget-wgpu`, and `models/hi.vm` are copied from Matt Keeter's
[Fidget](https://github.com/mkeeter/fidget), revision
`0c89e87e1b3a6d15cc0976ab6ff05a09f9cf91d6` (2026-09-12).
They retain upstream's [MPL-2.0 license](fidget-LICENSE.txt).
Cargo patches only these four packages; the remaining Fidget packages still use
that exact Git revision. Their manifests spell out upstream's inherited package
metadata and dependency versions, pinning core and workspace-hack to that revision.

Local source changes, each including regression tests:

- [Bulk float slice patch](../docs/experiments/fidget-core-bulk-slices.patch):
  handle input/output register aliases once per common arithmetic instruction,
  then operate on safely borrowed slices so LLVM can vectorize the loops.
  No unsafe code, extra allocation, sampling change, or altered math semantics.
  Covers aliases, tail lengths, signed zeros, infinities, and NaNs. This is
  independent of the other patches; see the
  [browser measurements](../docs/browser-fidget-bulk-profile-2026-09-19.md).
- [Shared cancellation flag patch](../docs/experiments/fidget-core-cancellation.patch):
  add `CancelToken::from_shared_flag(Arc<AtomicBool>)` so callers can share one
  cancellation signal across libraries without callback bridges. Existing token
  behavior is unchanged; the constructor preserves an already-cancelled flag.
  Progred uses this for both meshing and implicit rendering. This patch is
  independent of the others below.
- [Core SSA access patch](../docs/experiments/fidget-core-ssa.patch): expose the
  existing SSA tape, lower a caller-supplied valid tape with its variable map,
  and wrap a function as a shape. These additive APIs support the test-only
  persistent-expression experiment; they do not change existing evaluation or
  rendering. This patch is independent of the others below.
- [AArch64 JIT patch](../docs/experiments/fidget-jit-aarch64.patch): full-width
  spill addresses and stack adjustment, preservation of callee-saved registers,
  and long bulk-loop exits. Small tapes retain the short addressing form.
- [Raster cancellation patch](../docs/experiments/fidget-raster-cancellation.patch):
  pass the existing cancellation observation into subtile work in 2D and 3D;
  unwind incomplete tiles rather than publishing partial output. No public API,
  sampling, or geometry change.
- [Raster progress patch](../docs/experiments/fidget-raster-progress.patch):
  an optional serialized completed/total root-tile callback for voxel renders,
  excluding unfinished cancelled tiles. This applies after the cancellation
  patch; it does not change sampling or geometry.
- [Scene tiling patch](../docs/experiments/fidget-raster-scene.patch): render all
  objects within each image tile before counting its pixels as complete. It
  reuses the ordinary voxel worker and shared tile scheduler, keeps expressions
  separate, and preserves depth, normals, and first-object depth ties. Progress
  has a fixed image-pixel total, without timing estimates or object weights.
  This applies after the progress patch.
- [Scene refinement patch](../docs/experiments/fidget-raster-refinement.patch):
  a prepared scene retains root interval tapes between passes, workers reuse
  their per-object tile buffer, and an optional borrowed-tile callback exposes
  finished regions before whole-image assembly. Callbacks may run concurrently;
  consumers own publication/assembly policy. Applies after the scene tiling patch.
- [GPU spill experiment](../docs/experiments/fidget-wgpu-spills.patch): the normal
  backend rejects unsupported spilling geometry/color tapes. The non-default
  `experimental-spills` feature enables spill-aware simplification and bounded
  external scratch for voxel evaluation (pixel/color still use private arrays).
  Explicit experimental choice-history sizing supports diagnosis. Full-stock
  tests now match the CPU without exceeding Metal's private stack, but remain
  substantially slower; general limits are not established. The headless
  stage/tape profiler identifies program-arena exhaustion and can test a larger
  arena without changing the renderer's default. Voxel work now excludes
  offscreen X/Y padding. A diagnostic screen-region batching experiment bounds
  program lifetimes without changing sampling density, but repeats enough
  interval work to lose performance; it remains outside the app.
  **Not ready for app use or an upstream proposal.** See the
  [experiment report](../docs/fidget-gpu-experiment-2026-09-16.md). Its tests use
  the core SSA access patch above.

These patch files apply to upstream in the order listed, with no manifest adaptation, and are
intended for upstream review, except the explicitly experimental GPU patch.
Nothing has been submitted or pushed upstream.
Keep vendored source and patches in sync; do not accumulate unrelated changes.
Remove each local package override when a reviewed upstream revision includes
the corresponding fixes. See the [measurements](../docs/cam-render-profiling-2026-09-15.md).

The packages are workspace members so their tests share our locked dependency
graph and sandboxed build. On a supported desktop JIT platform:

```sh
./tools/sandbox-cargo test --release -p fidget-core -p fidget-jit -p fidget-raster --lib
```

For iOS/Web builds, select `-p progred` as the normal platform targets do: the
JIT package itself does not support those platforms. Progred enables JIT only
on Apple Silicon macOS, using its native recommended raster tiles. Other
platforms and all meshing retain the VM. The macOS bundle requires the standard
allow-JIT entitlement; App Sandbox and hardened runtime remain enabled.
