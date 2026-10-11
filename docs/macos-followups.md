# macOS follow-ups

Changes from the October 2026 cloud sessions that were built and tested on
Linux only.
Check each on macOS, then delete it from this list; delete the file when it's
empty.

## Check that these still work

- **`make check` and `make test`.** They replace `make sandbox-check` and
  `make sandbox-test` and should run the same `sandbox-cargo` commands. The
  Clippy cleanup, the shared test driver, and the experiment retirement were
  tested only through `tools/linux-cargo`.
- **Keys in the app.** Paste, delete, insert, replace, Select All, and arrow
  navigation moved from shell code into the frame's lowest-priority handler.
  Try arrows, Backspace and Delete, Enter, Command+V, and Command+A in `make run`.
- **`make build-web` and `make run-web`.** The browser build now reads its Cargo
  and wasm-bindgen arguments from `tools/web-build-settings.sh`. `run-web`
  replaces `run-website` and serves on port 8081; the editor should load at
  `http://127.0.0.1:8081/editor/`.
- **`make images`.** It now uses the platform's wrapper and build directory,
  which on macOS should be the same `target/sandbox/build`.
- **The delimiter bench.** `./tools/sandbox-cargo run -p puri --example
  delimiter_bench` should now write `target/sandbox/build/delimiter_bench.svg`
  instead of failing outside the sandbox's writable folders.

## The Fidget 0.5.1, Vello 0.11, and wgpu 30 upgrade

Linux checked the native build, the threaded browser build's types, `make test`,
and the GPU tests on Mesa's lavapipe software Vulkan driver; nothing ran on
Metal or Apple Silicon.

- **Rust 1.95 or newer.** Fidget 0.5.1 requires it; run `rustup update stable`,
  then `make fetch` for the new crates.
- **The AArch64 JIT patch.** It was re-applied to 0.5.1 but only compiles on
  Apple Silicon. Run the vendored tests:
  `./tools/sandbox-cargo test --release -p fidget-core -p fidget-jit -p fidget-raster --lib`.
- **Presentation and color.** Windows now present through `Queue::present`, and
  wgpu 30 changed Metal color-space handling. Check drawing, resizing, and
  colors in `make run`.
- **Text.** Parley 0.11 brings a newer shaping engine and Vello 0.11 fixes emoji
  placement; compare `make images` output and the app's text for drift.
- **Browser.** Check `make run-web` with WebGPU, and run
  `node tools/test-web-gpu.cjs` as the [browser notes](../web/README.md#diagnostics)
  describe.

## GPU tests

The readback helper moved from `progred/tests/compositor_experiment/gpu.rs` into
`puri-vello`'s `capture` module, behind a `capture` feature. The GPU tests pass
on lavapipe; run them on Metal as [testing](testing.md#ignored-tests)
describes.

## Simplifications to make where they can be tested

- **`tools/sandbox-cargo`.** Lines 33–36 repeat what `use_toolchain` does;
  define the function first and call `use_toolchain stable`. Every
  `run_sandboxed` call passes `allow_network` and `offline` as opposites, so
  derive one from the other.
- **`tools/build-macos-app`.** `--identifier` repeats `CFBundleIdentifier` from
  `macos/Info.plist`, which codesign reads from the bundle anyway.
- **`website/Preview.command`.** It adds rustup locations to `PATH` that
  `sandbox-cargo` already searches; check whether anything else needs them.
- **Rust 1.100, due 12 November 2026.** Move `sandbox-cargo update` and
  `resolve` to stable Cargo, where `registry.global-min-publish-age` no longer
  needs `-Z min-publish-age`, and merge their near-identical branches. See
  [build security](build-security.md).
