# Testing

`make test` runs the workspace's tests through the platform's Cargo wrapper (see
[build security](build-security.md)). To run a subset, pass Cargo's usual
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
  navigation, and the phase-ordering tests that check placement doesn't run
  hover and hover doesn't paint or dispatch.
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
  CAM work; see [performance checks](performance.md).
- **GPU tests** need a real adapter. Seatbelt provides none, so on macOS build
  the test binaries in the sandbox and run the executables Cargo prints
  directly:

  ```sh
  ./tools/sandbox-cargo test --release -p puri-vello --features capture --test compositor --no-run
  ./tools/sandbox-cargo test --release -p progred --lib --no-run
  ```

  Run the first executable with `--ignored --test-threads=1`, and the second
  with `compositor --ignored --test-threads=1`, or with a test name such as
  `gpu_colors_constants_and_workspace_reuse`. On Linux, Cargo can run them
  directly:

  ```sh
  ./tools/linux-cargo test --release -p puri-vello --features capture --test compositor -- --ignored --test-threads=1
  ./tools/linux-cargo test --release -p progred --lib compositor -- --ignored --test-threads=1
  ```

  The readback helper is `puri-vello`'s `capture` module, behind its `capture`
  feature.
- **Experiments** such as the analytic tool sweep stay test-only; their notes
  are in [history](history/README.md).
