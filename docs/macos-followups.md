# macOS follow-ups

Changes from the 2026-10-10 cleanup that were built and tested on Linux only.
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

## GPU tests

These need real GPU access, which Seatbelt doesn't provide. Build the test
binaries in the sandbox, then run the executables Cargo prints directly. The
readback helper moved from `progred/tests/compositor_experiment/gpu.rs` into
`puri-vello`'s `capture` module, behind a `capture` feature.

```sh
./tools/sandbox-cargo test --release -p puri-vello --features capture --test compositor --no-run
./tools/sandbox-cargo test --release -p progred --lib --no-run
```

Run the first executable with `--ignored --test-threads=1`, and the second with
`compositor --ignored --test-threads=1`.

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

## Decisions

- **Unused vendored Fidget patches.** Progred no longer uses the core SSA access
  patch or the GPU spill patch; their only users were retired experiments. Keep
  them for a possible upstream proposal, or drop them from `vendor/` and
  [vendor/README.md](../vendor/README.md).
