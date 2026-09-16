# Local Fidget patches

`fidget-jit`, `fidget-raster`, and `models/hi.vm` are copied from Matt Keeter's
[Fidget](https://github.com/mkeeter/fidget), revision
`0c89e87e1b3a6d15cc0976ab6ff05a09f9cf91d6` (2026-09-12).
They retain upstream's [MPL-2.0 license](fidget-LICENSE.txt).
Cargo patches only these two packages; the remaining Fidget packages still use
that exact Git revision. Their manifests spell out upstream's inherited package
metadata and dependency versions, pinning core and workspace-hack to that revision.

Local source changes, each including regression tests:

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

These patch files apply to upstream in the order listed, with no manifest adaptation, and are
intended for upstream review. Nothing has been submitted or pushed upstream.
Keep vendored source and patches in sync; do not accumulate unrelated changes.
Remove each local package override when a reviewed upstream revision includes
the corresponding fixes. See the [measurements](../docs/cam-render-profiling-2026-09-15.md).

The packages are workspace members so their tests share our locked dependency
graph and sandboxed build. On a supported desktop JIT platform:

```sh
./tools/sandbox-cargo test --release -p fidget-jit -p fidget-raster --lib
```

For iOS/Web builds, select `-p progred` as the normal platform targets do: the
JIT package itself does not support those platforms. Progred enables JIT only
on Apple Silicon macOS, using its native recommended raster tiles. Other
platforms and all meshing retain the VM. The macOS bundle requires the standard
allow-JIT entitlement; App Sandbox and hardened runtime remain enabled.
