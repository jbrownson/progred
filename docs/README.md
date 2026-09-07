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
- [Puri and the editor frame](puri.md): ownership, layout, events, hover, and
  drawing.
- [Build security](build-security.md): sandboxed build/test commands.
- [Platforms](platforms.md): the native and browser hosts.

[Deferred work](deferred.md) records the unresolved items set aside during the
review. [Historical notes](history/README.md) preserve earlier models and
proposals separately; they are not required reading for ordinary changes.
[Layout continuations](layout-continuations.md) describes the box/widget
boundary, the frame stages, and their verification.

[Performance checks](performance.md) describes the shared headless frame harness
and the IoP/Fidget canaries. [Tree profiling](tree-profile-2026-09-04.md) is a
dated measurement report.
[Library resolution](library-resolution-2026-09-05.md) records the later
definition-storage change and its performance checks.
The [release checklist](release-checklist.md) covers distribution concerns.

[Examples](../examples/README.md) lists the bundled documents, shortcuts, and
the formulas behind the Fidget shapes.
