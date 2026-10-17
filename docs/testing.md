# Testing

`make test` runs the workspace's unit and integration tests through the
platform's Cargo wrapper (see [build security](build-security.md)), except the
vendored Fidget packages'. Those are upstream's tests of code we rarely change;
`make test-vendor` runs them, optimized, after a patch changes or Fidget is
upgraded. Only progred's library has tests, so it compiles once, as its test
harness; `make check` compiles the binaries and examples. Development builds
optimize every crate except progred, which stays quick to rebuild, so tests
spend little time in Fidget, Vello, text shaping, Grap, or Puri; once built,
`make test` takes about 40 s on a 4-core Linux VM. Progred's tests lay out text
with the bundled Noto fonts, so layout is the same on every machine; a few of
Puri's still use system fonts. While iterating, run a subset by passing Cargo's
usual arguments through `./tools/sandbox-cargo test` on macOS or
`./tools/linux-cargo test` on Linux, for example
`./tools/linux-cargo test -p progred --lib tutorials`, and leave `make test` for
before pushing. Each feature set and profile keeps its own build of progred, and
the builds pile up. On 17 October 2026 they filled a cloud session's disk with
16 GB. `./tools/linux-cargo clean -p progred` removes them and leaves the
optimized dependencies in place. Nothing here needs a window; the owner does the
visual testing.

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
  and `mouse`. Some load the small documents in
  [`progred/fixtures`](../progred/fixtures).
- **Interactions.** [`crate::driver::Driver`](../progred/src/driver.rs) drives
  the editor as a person would: pointer and modifier input arrives as Winit
  window events through the shell's own translation, keys as the platform
  delivers them, and the queries in
  [`driver/inspect.rs`](../progred/src/driver/inspect.rs) read back what is
  drawn where by document path. The tutorial tests follow the tutorial page's
  steps with it, and the menu interaction tests use it too. Its input also
  serves the [session an agent drives](#a-session-an-agent-drives).
- **Browser.** The website's server and scripts have their own tests, and
  headless Chrome diagnostics measure the browser build; see the
  [website](../website/README.md) and [browser host](../web/README.md) notes.
- **Vendored Fidget.** Its packages are workspace members with their own tests;
  see [vendor/README.md](../vendor/README.md).

## Pictures

`make test` includes the `svg_bench` tests, which render the sample, the Grap
demo, and other fixtures to SVG in the build directory, `target/sandbox/build`.
The ignored `editor_svg_captures` and
progressive CAM captures paint whole editor frames; see
[performance checks](performance.md#headless-editor-captures). `make images`
regenerates the README and website screenshots. The `delimiter_bench` example
of `puri` renders the drawn delimiters against the font's outlines. Convert
SVGs to PNG with `rsvg-convert` from librsvg.

These are layout and content captures, not pixel-exact Vello output: Fidget
falls back to the CPU without a GPU, glyphs are unhinted outlines, and the
exporter supports solid brushes only.

## A session an agent drives

`make control ARGS='--document examples/grap-demo.gid'` builds the
`progred-control` binary and serves one headless editor on
`target/control/progred.sock`, which only its owner can connect to. Its window
is 1280 × 800 pixels, one per logical unit at actual size, with the in-app
menus, and it keeps a clipboard of its own. The app's own menu items (New, Open,
the examples, windows) do nothing there, and saving an untitled document asks
for no path.

`tools/control` sends it one action at a time, named and shaped as in
Anthropic's computer-use tool: `screenshot`, `mouse_move`, `left_click`,
`double_click`, `triple_click`, `left_click_drag`, `scroll`, `key` and `type`.
Screenshots are saved as PNGs under `target/control/screenshots`. Each request
is a GID document whose root calls the session's
[`computer` library](../progred/src/control/computer.rs), and each reply a
document whose root is the result: `done`, `{png: …}`, or an absent that says
why the action couldn't happen. The session writes the vocabulary's binders to
`target/control/vocabulary.gid` for clients to spell requests with. Its first
use is to see how an agent that knows nothing of the code copes with the
editor; see [deferred work](deferred.md#external-control-and-a-newcomer-test).

`tools/web-control` takes the same actions, plus `wait`, on the website.
`tools/web-control serve tutorial.html` builds and serves the site through the
preview server, opens the page in a 1280 × 800 headless Chrome, or the browser
at `$CHROMIUM`, and listens on `target/web-control/browser.sock`; the other
commands each send it one action. An agent can then take a tutorial as a reader
does, with its prose, steps and checks. It thinks aloud with `say`, and every
action goes to `target/web-control/session.jsonl` with its timing. Setting
`WEB_CONTROL_DIR` moves the socket, log and screenshots elsewhere, so two
sessions can run side by side.
`tools/playtest-video` replays that log in a fresh browser at a pace a person
can follow and writes a video beside it: a drawn pointer, the notes spoken and
captioned under the page, and each action named. It needs ffmpeg, Playwright's
ffmpeg for recording, and a voice. `tools/playtest-voice setup` fetches Kokoro,
an open speech model that runs on the CPU faster than it speaks, into
`target/playtest-voice`: about 120 MB of model from the kokoro-onnx release and
a virtual environment with kokoro-onnx. Without it, the video falls back to
`pico2wave` from libttspico-utils, macOS's `say`, or `espeak-ng`, sped up twice
over to Kokoro's pace.

## Ignored tests

Ignored tests are opt-in: run them with `-- --ignored`, usually with
`--release` and `--nocapture`.

- **Performance canaries and profiles** measure headless frame construction and
  CAM work. `make perf` runs the canaries and compares them with the
  [performance log](performance-log.tsv); see
  [performance checks](performance.md). Puri's
  `handler_dispatch_profile` checks the cost of dispatching through unrelated
  handlers.
- **GPU tests** need a real adapter: `make test-gpu` runs the compositor tests
  and Fidget's GPU raster and mesh tests. The build sandbox provides no GPU, so
  [`tools/test_binaries.py`](../tools/test_binaries.py) builds the tests through
  the wrapper and runs the executables outside it; it takes `cargo test`
  arguments, then test arguments after `--`. On Linux without a GPU, Mesa's
  lavapipe driver works. The readback helper is `puri-vello`'s `capture` module,
  behind its `capture` feature.
- **Browser presentation** has its own pixel test: `make test-web-gpu` builds the
  GPU probe, then drives the installed Chrome headlessly through the
  `playwright-core` pinned in `tools/package.json`, on WebGPU and on the
  Canvas2D fallback.
- **Experiments** such as the analytic tool sweep stay test-only; their notes
  are in [history](history/README.md).
