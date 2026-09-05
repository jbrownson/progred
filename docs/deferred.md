# Deferred work

These are open questions carried forward from the review, not tasks scheduled
for the next cleanup. The current implementation is documented in
[model.md](model.md), [projections.md](projections.md), and [puri.md](puri.md).

## Active insertion hidden by a specialized projection

A specialized projection can omit the structure where an insertion is pending.
The general interaction and presentation policy needs dedicated design work.
Do not hide a fallback policy in reusable widgets or infer it by inspecting a
render tree. This was explicitly set aside for another day.

## Address lifetime

Paths and source traces already preserve definition source. A future identity
review should specify what survives list moves, deletion/undo, definition
replacement, and transient-root changes. A cell has meaning in a resolution
context; neither a path nor a cell-relative trace is a global address.
Keep occurrence identity, cell identity, definition source, and view identity
distinct while answering those questions.

## Reporting completion failures

Offer construction decides applicability; activating an offered completion
consumes the input. Its Grap continuation can still fail at runtime. For now,
keep the existing silent failure: unsuccessful preparation installs neither
the insertion nor its staged selection/annotation changes.

Later, design an editor-owned way to surface these failures, such as alerts or
notifications, using the failure details in GID values. Do not add that machinery
now. Distinguishing an actual failure or explicit decline from a legitimate
absent result remains an open semantic question; absence alone should not be
assumed to warrant an alert.

## General computation reuse

The canvas memo was removed because its validation observed less than evaluation
could use. Current canvas programs record once per visible frame; text shaping
is the cross-frame memo. Any future invalidation system must track actual
lookup results, including absence, ordered definition sets, and foreign
implementations. Settle the model before adding it. Do not reintroduce an
ad-hoc cache or event-specific relevance checks.

The [2026-09-04 tree profile](tree-profile-2026-09-04.md) records measurements
from that implementation; timings are historical observations, not guarantees.

## Platform work

The iOS host remains a feasibility prototype. Further product work is deferred
while macOS/Linux mature. See [platform notes](platforms.md) for its current
build path and limitations. Older visionOS and language-extension ideas remain
uncommitted possibilities in the historical notes.
