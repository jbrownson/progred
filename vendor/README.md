# Local Fidget patches

`fidget-core`, `fidget-jit`, `fidget-raster`, and `fidget-wgpu` are copied
from Matt Keeter's [Fidget](https://github.com/mkeeter/fidget) 0.5.1 as
published on crates.io, whose sources match upstream's `v0.5.1` tag;
`models/hi.vm` comes from the same tag. They retain upstream's
[MPL-2.0 license](fidget-LICENSE.txt). Cargo patches only these four packages
over crates.io; the remaining Fidget packages are the published 0.5.1. Their
manifests spell out upstream's inherited package metadata and dependency
versions.

Local source changes, each including regression tests:

- [Bulk float slice patch](patches/fidget-core-bulk-slices.patch):
  handle input/output register aliases once per common arithmetic instruction,
  then operate on safely borrowed slices so LLVM can vectorize the loops.
  No unsafe code, extra allocation, sampling change, or altered math semantics.
  Covers aliases, tail lengths, signed zeros, infinities, and NaNs. This is
  independent of the other patches; see the
  [browser measurements](../docs/history/browser-fidget-bulk-profile-2026-09-19.md).
- [Shared cancellation flag patch](patches/fidget-core-cancellation.patch):
  add `CancelToken::from_shared_flag(Arc<AtomicBool>)` so callers can share one
  cancellation signal across libraries without callback bridges. Existing token
  behavior is unchanged; the constructor preserves an already-cancelled flag.
  Progred uses this for both meshing and implicit rendering. This patch is
  independent of the others below.
- [AArch64 JIT patch](patches/fidget-jit-aarch64.patch): full-width
  spill addresses and stack adjustment, preservation of callee-saved registers,
  and long bulk-loop exits. Small tapes retain the short addressing form.
- [Raster cancellation patch](patches/fidget-raster-cancellation.patch):
  pass the existing cancellation observation into subtile work in 2D and 3D;
  unwind incomplete tiles rather than publishing partial output. No public API,
  sampling, or geometry change.
- [Raster progress patch](patches/fidget-raster-progress.patch):
  an optional serialized completed/total root-tile callback for voxel renders,
  excluding unfinished cancelled tiles. This applies after the cancellation
  patch; it does not change sampling or geometry.
- [Scene tiling patch](patches/fidget-raster-scene.patch): render all
  objects within each image tile before counting its pixels as complete. It
  reuses the ordinary voxel worker and shared tile scheduler, keeps expressions
  separate, and preserves depth, normals, and first-object depth ties. Progress
  has a fixed image-pixel total, without timing estimates or object weights.
  This applies after the progress patch.
- [Scene refinement patch](patches/fidget-raster-refinement.patch):
  a prepared scene retains root interval tapes between passes, workers reuse
  their per-object tile buffer, and an optional borrowed-tile callback exposes
  finished regions before whole-image assembly. Callbacks may run concurrently;
  consumers own publication/assembly policy. Applies after the scene tiling patch.
- [GPU spill rejection patch](patches/fidget-wgpu-spill-rejection.patch):
  `RenderShape::new` and `ShapeColorBuffers::new` reject tapes that need
  register spills with `SpillsUnsupported`. Upstream accepts them, but its GPU
  interpreter leaves spill loads and stores unimplemented, so they render
  nothing. Progred falls back to the CPU on this error. An earlier experiment
  implemented GPU spills instead; see its
  [report](../docs/history/fidget-gpu-experiment-2026-09-16.md).

These patch files apply to the published 0.5.1 sources in the order listed, with
no manifest adaptation, and are intended for upstream review. Nothing has been
submitted or pushed upstream.
Keep vendored source and patches in sync; do not accumulate unrelated changes.
Remove each local package override when a reviewed upstream release includes
the corresponding fixes. See the [measurements](../docs/history/cam-render-profiling-2026-09-15.md).

The packages are workspace members so their tests share our locked dependency
graph and sandboxed build. On a supported desktop JIT platform:

```sh
./tools/sandbox-cargo test --release -p fidget-core -p fidget-jit -p fidget-raster --lib
```

For browser builds, select `-p progred` as the normal platform targets do: the
JIT package itself does not support those platforms. Progred enables JIT only
on Apple Silicon macOS, using its native recommended raster tiles. Other
platforms and all meshing retain the VM. The macOS bundle requires the standard
allow-JIT entitlement; App Sandbox and hardened runtime remain enabled.
