# Historical notes

These files preserve earlier implementation reports, experiments, proposals,
and rationale. They are not current instructions. Much of the prose was written
by assistants; an attribution or statement of intent here has not been verified
as the owner's view. Dates describe when a note was written or revised, not a
promise that its claims remain true.

The 2026-09-04 cleanup preserved snapshots from commit `7da4bb0`:

- [Data/editor model notes](model-notes.md), including the superseded typed and
  atomic models and UI-state migration narratives.
- [Puri notes](puri-notes.md), including framework comparisons, discarded layout
  and handler designs, and proposed ecosystem work.
- [Grap/projection notes](projections-notes.md), including language proposals
  and the removed wasm experiment.

The [neighbor callback navigation experiment](navigation-neighbor-experiment-2026-09-26.md)
records the implementation replaced by logical-layout navigation on 2026-09-27.
The [layout navigation measurements](navigation-layout-2026-09.md) record
timings and prototype notes for the collector revisions that followed.

Line controls were once implemented in Grap. That work showed Grap controls can
use shaped geometry, generic events, and scoped site/selection capabilities,
but duplicating Puri's editor composition there had reached diminishing
returns, so text and numbers moved to the stock native line widget. (Recorded
from AGENTS.md on 2026-09-29.)

The [iPad host notes](ipad-host.md) describe the feasibility port removed on
2026-09-29.

The [Roc comparison notes](roc-notes.md) retain the former porting backlog
as proposals rather than current tasks.

## Dated measurements and experiments, September 2026

Moved here from `docs/` on 2026-10-10. Each records what was measured or tried
at the time; code some of them describe has since been retired.

- [Performance log](performance-log-2026-09.md): the dated frame, evaluator, CAM,
  and layout measurements formerly in the performance checks.
- [Tree profiling](tree-profile-2026-09-04.md) and
  [library resolution](library-resolution-2026-09-05.md): early frame and
  definition-storage measurements.
- [Fidget meshing](fidget-meshing-2026-09-13.md): the headless cube meshing
  experiment and its quality limits.
- [CAM rendering investigation](cam-render-profiling-2026-09-15.md): stock,
  tool, path, mesh, and progressive-render costs, and the local JIT fix.
- [Retained progressive rendering](fidget-progressive-experiment-2026-09-15.md):
  whole-view, bounded-tile, program-sharing, subtree census, and
  persistent-expression measurements.
- [GPU spills](fidget-gpu-experiment-2026-09-16.md): the paused GPU interpreter
  experiment, CPU comparisons, and batching tradeoffs.
- [Graphics memory](graphics-memory-2026-09-16.md): the idle-footprint
  investigation and Vello resource retention.
- [Hybrid CAM rendering](fidget-hybrid-2026-09-18.md): implicit model and stock
  color and depth combined with mesh paths and cutters.
- [Separate image composition](vello-compositor-experiment-2026-09-18.md): the
  compositor experiment and its integration around Vello's image atlas.
- [Browser worker experiment](web-worker-experiment-2026-09-19.md): the
  shared-memory probe and the synchronization fixes behind the integrated worker.
- [Browser/native profiling](browser-native-profile-2026-09-19.md),
  [SIMD](browser-simd-profile-2026-09-19.md),
  [Fidget bulk loops](browser-fidget-bulk-profile-2026-09-19.md),
  [orbit profiling](browser-orbit-profile-2026-09-19.md), and
  [allocators](browser-allocator-experiment-2026-09-19.md): browser measurements
  of the CAM document, made with the diagnostics in the
  [browser README](../../web/README.md#diagnostics).
- [Native CAM source scrolling](native-scroll-profile-2026-09-26.md): source-frame
  cost around the navigation rewrite.
- [Analytic tool sweep](tool-sweep-experiment-2026-09-15.md): the test-only
  bull-mill candidate and why it hasn't replaced the working approximation.
- [Fidget's JIT against its VM](fidget-jit-vm-2026-10-11.md): the Apple Silicon
  measurements that made the VM the software evaluator everywhere.
- [Dependency reviews](dependency-reviews-2026.md): the Fidget Git pin, vendoring
  and JIT, publication-age exceptions, and the constant-field upstream check,
  moved from build security and the release checklist on 2026-10-11.
- [Codebase audit](codebase-audit-2026-10-16.md): where progred's size is,
  duplicated test scaffolding, and the dependency cycle that blocks a crate
  split, with ranked recommendations.
- [Crate split analysis](crate-split-2026-10-16.md): the three crates the audit's
  split would become, the coupling across their boundaries, and the steps that
  break it.

## Earlier notes

The [lambda calculus exploration](lambda-calculus-exploration.md) records the
July 2026 line of thought, dead ends included, behind the values-and-cells model.

The [Rust pivot](rust-pivot.md) and [prototype survey](prototype-survey.md)
are older handoffs. Use [current documentation](../README.md) for the active
implementation, and confirm behavior in code and tests when making changes.
