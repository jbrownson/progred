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

The [Rust pivot](rust-pivot.md) and [prototype survey](prototype-survey.md)
are older handoffs. Use [current documentation](../README.md) for the active
implementation, and confirm behavior in code and tests when making changes.
