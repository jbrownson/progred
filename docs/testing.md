# Testing

`make test` runs the workspace's tests through the platform's Cargo wrapper (see
[build security](build-security.md)), except the vendored Fidget packages'.
Those are upstream's tests of code we rarely change; `make test-vendor` runs
them, optimized, after a patch changes or Fidget is upgraded. Development builds
optimize every crate except progred, which stays quick to rebuild, so tests
spend little time in Fidget, Vello, text shaping, Grap, or Puri; `make test`
takes about 30 s on a 4-core Linux VM. Tests lay out text with the bundled Noto
fonts, so layout is the same on every machine. To run a subset, pass Cargo's usual
arguments through `./tools/sandbox-cargo test` on macOS or
`./tools/linux-cargo test` on Linux, for example
`./tools/linux-cargo test -p progred --lib tutorials`. Nothing here needs a
window; the owner does the visual testing.

## Layers

- **Pure logic.** GID, Grap, `incremental`, layout, and the libraries test their
  functions directly. Prefer these where they can say what's being tested.
- **Frames.** [`projection/tests`](../progred/src/projection/tests/mod.rs)
  builds whole headless frames from an editor state and inspects their draw
  lists, hover, and handlers: projection fixtures, interaction regressions,
  navigation, and phase-ordering tests: placement doesn't run hover, hover
  doesn't paint or dispatch, paint sees the final target, and discarding paint
  leaves a working handler. Projection-shape tests record layout calls and
  projection recursion through test-only recorders that production doesn't
  use.
  [`display/test_support.rs`](../progred/src/display/test_support.rs) records
  projection calls and supplies small helpers such as `painted`, `Landmarks`,
  and `mouse`.
- **Interactions.** [`crate::driver::Driver`](../progred/src/driver.rs) drives
  the editor as a person would: pointer and modifier input arrives as Winit
  window events through the shell's own translation, keys as the platform
  delivers them, and queries read back what is drawn where by document path.
  The tutorial tests follow each tutorial page's steps with it, and the menu
  interaction tests use it too.
- **Browser.** The website's server and scripts have their own tests, and
  headless Chrome diagnostics measure the browser build; see the
  [website](../website/README.md) and [browser host](../web/README.md) notes.
- **Vendored Fidget.** Its packages are workspace members with their own tests;
  see [vendor/README.md](../vendor/README.md).

## Pictures

`make test` includes the `svg_bench` tests, which render the sample, the Grap
demo, and other fixtures to SVG in the build directory (`target/sandbox/build`
on macOS, `target/native` on Linux). The ignored `editor_svg_captures` and
progressive CAM captures paint whole editor frames; see
[performance checks](performance.md#headless-editor-captures). `make images`
regenerates the README and website screenshots. The `delimiter_bench` example
of `puri` renders the drawn delimiters against the font's outlines. Convert
SVGs to PNG with `rsvg-convert` from librsvg.

These are layout and content captures, not pixel-exact Vello output: Fidget
falls back to the CPU without a GPU, glyphs are unhinted outlines, and the
exporter supports solid brushes only.

## Ignored tests

Ignored tests are opt-in: run them with `-- --ignored`, usually with
`--release` and `--nocapture`.

- **Performance canaries and profiles** measure headless frame construction and
  CAM work. `make perf` runs the canaries and compares them with the
  [performance log](performance-log.tsv); see
  [performance checks](performance.md). Puri's
  `handler_dispatch_profile` checks the cost of dispatching through unrelated
  handlers.
- **GPU tests** need a real adapter: `make test-gpu` runs the compositor
  tests and Fidget's GPU raster and mesh tests. Seatbelt provides no GPU, so
  [`tools/test_binaries.py`](../tools/test_binaries.py) builds the tests through
  the wrapper and runs the executables outside it; it takes `cargo test`
  arguments, then test arguments after `--`. On Linux without a GPU, Mesa's
  lavapipe driver works. The readback helper is `puri-vello`'s `capture`
  module, behind its `capture` feature.
- **Browser presentation** has its own pixel test: `make test-web-gpu` builds the
  GPU probe, then drives the installed Chrome headlessly through the
  `playwright-core` pinned in `tools/package.json`, on WebGPU and on the
  Canvas2D fallback.
- **Experiments** such as the analytic tool sweep stay test-only; their notes
  are in [history](history/README.md).
