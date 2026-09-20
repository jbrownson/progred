# Documentation

The current reference describes the implementation; it is not evidence that
the owner endorsed every sentence. Existing prose was largely assistant-written.
When a description conflicts with code, investigate the difference instead of
restoring an older design automatically. Explicit owner instructions take
precedence over inferred intent in documentation.

Start with:

- [GID](gid.md): the native logical substrate and the boundary with the
  [temporary text bridge](gid-text.md).
- [Data and editor model](model.md): addresses, selection, completion, panes,
  history, and persistence.
- [Grap and projections](projections.md): evaluation, libraries, display
  composition, and host boundaries.
- [Dependency-tracked computations](incremental.md): memo graphs, Grap observations,
  cancellable background jobs, and the first CAM integration.
- [Puri and the editor frame](puri.md): ownership, layout, events, hover, and
  drawing.
- [Build security](build-security.md): sandboxed build/test commands.
- [Platforms](platforms.md): the native and browser hosts.

[Browser worker experiment](web-worker-experiment-2026-09-19.md) records the
shared-memory closure probe, the UI-thread synchronization fixes, and the
browser executor/Rayon integration. [Browser setup](../web/README.md) covers the host.
[Browser/native profiling](browser-native-profile-2026-09-19.md) compares the
actual CAM document's computation and retained mesh drawing on both backends,
including the browser worker-pool and WebGPU improvements.
[Browser SIMD profiling](browser-simd-profile-2026-09-19.md) measures the smaller
compiler-flag improvement and checks final color/depth outputs.
[Fidget bulk-loop experiment](browser-fidget-bulk-profile-2026-09-19.md) profiles
the actual browser rendering workers and tests a safe slice-loop rewrite that
enables vectorization; the measured patch is now applied to the vendored core.
[Browser orbit profiling](browser-orbit-profile-2026-09-19.md) follows the actual
frame/gesture path and separates source-pane construction from background
worker contention.
[Allocator experiments](browser-allocator-experiment-2026-09-19.md) compare two
alternative WASM allocator configurations against that same orbit replay.

[Deferred work](deferred.md) records the unresolved items set aside during the
review. [Historical notes](history/README.md) preserve earlier models and
proposals separately; they are not required reading for ordinary changes.
[Layout continuations](layout-continuations.md) describes the box/widget
boundary, the frame stages, and their verification.

[Performance checks](performance.md) describes the shared headless frame harness
and the IoP/Fidget canaries. [Tree profiling](tree-profile-2026-09-04.md) is a
dated measurement report.
[Fidget meshing](fidget-meshing-2026-09-13.md) records the headless cube experiment
and its quality limitations. The [mesh viewport](fidget-mesh.md) describes the
subsequent in-app option. Its toolpath variant now retains dependency-tracked
geometry; ordinary mesh previews still remesh each frame.
[Retained progressive rendering](fidget-progressive-experiment-2026-09-15.md)
records test-only whole-view, bounded-tile, exact-program-sharing, subtree
census, and persistent-expression measurements. CAM now tries mesh → final-quality
implicit tiles; the standalone implicit preview retains independent refinement passes.
[Hybrid CAM rendering](fidget-hybrid-2026-09-18.md) records the subsequent split:
implicit model/stock color and depth combined with mesh paths and cutters.
[Graphics memory](graphics-memory-2026-09-16.md) records the idle-footprint
investigation and isolated Vello resource-retention measurements.
[Separate image composition](vello-compositor-experiment-2026-09-18.md) records the
experiment and native integration around Vello's image atlas, including clipping,
upload reuse, GPU-texture handoff, and single-/multiple-preview presentation costs.
Direct GPU mesh drawing now uses that same ordered compositor on native and
WebGPU; the browser retains Canvas2D/CPU drawing as a fallback.
[GPU spills](fidget-gpu-experiment-2026-09-16.md) records the paused, opt-in GPU
interpreter experiment, CPU comparisons, program-arena pressure, and batching
tradeoffs. CAM remains on the CPU renderer.
[Library resolution](library-resolution-2026-09-05.md) records the later
definition-storage change and its performance checks.
The [release checklist](release-checklist.md) covers distribution concerns.

[Examples](../examples/README.md) lists the bundled documents, shortcuts, and
the formulas behind the Fidget shapes.
[Toolpaths](toolpaths.md) describes the first streaming CAM geometry library
and its Rhino-derived combined model-and-path preview.
[Tool profiles](tool-profiles.md) describes revolved cutter sections and the
remaining tool-change/chamfer work.
[Tool-sweep experiment](tool-sweep-experiment.md) records the test-only analytic
bull-mill candidate and why it has not replaced the working approximation.
[Controls](controls.md) describes reusable sliders and Grap control composition
used by toolpath playback.
[Final-encoded trees](trees.md) describes scoped hierarchy construction and
source-linked collection shared by the CAM controls and preview.
