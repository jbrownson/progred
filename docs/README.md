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
- [Build security](build-security.md): the build sandbox and dependency policy.
- [Platforms](platforms.md): building and running on macOS, Linux, and the
  browser.

Working on the editor:

- [Layout continuations](layout-continuations.md): the box/widget boundary and
  the frame stages.
- [Navigation](navigation.md): logical-line navigation and its timing canary.
- [Controls](controls.md): reusable sliders and Grap control composition used by
  toolpath playback.
- [Final-encoded trees](trees.md): scoped hierarchy construction and
  source-linked collection shared by the CAM controls and preview.
- [Performance checks](performance.md): the shared headless frame harness and
  its canaries.
- [Browser setup](../web/README.md): the browser host and its diagnostics.

CAM and Fidget:

- [Examples](../examples/README.md): the bundled documents, shortcuts, and the
  formulas behind the Fidget shapes.
- [Fidget cube](fidget-cube.md): the Rhino-derived cube model and its standalone
  regression fixture.
- [Mesh viewport](fidget-mesh.md): in-app meshing. The toolpath variant retains
  dependency-tracked geometry; ordinary mesh previews remesh each frame.
- [Toolpaths](toolpaths.md): the streaming CAM geometry library and its combined
  model-and-path preview.
- [Tool profiles](tool-profiles.md): revolved cutter sections and the square
  tool's chamfer strategies.
- [Tool-sweep experiment](tool-sweep-experiment.md): the test-only analytic
  bull-mill candidate and why it hasn't replaced the working approximation.

Also:

- [Deferred work](deferred.md): open questions and unscheduled next steps.
- [Release checklist](release-checklist.md): distribution concerns.
- [macOS follow-ups](macos-followups.md): cleanup changes still to check on macOS.
- [History](history/README.md): earlier models and proposals, and dated
  measurements and experiments. Not required reading for ordinary changes.
