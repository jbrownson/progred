# macOS follow-ups

Changes from the October 2026 cloud sessions that were built and tested on
Linux only.
Check each on macOS, then delete it from this list; delete the file when it's
empty.

## Next

- **The browser build's shared commands.** `web-threaded` and
  `web-threaded-fetch` moved from `sandbox-cargo` into
  `tools/cargo-common.sh`, so Linux builds the browser editor too, and
  `make build-web` calls the platform's wrapper. Run
  `./tools/sandbox-cargo web-threaded-fetch` and `make run-web`.
- **The clipboard.** The system clipboard's copy and paste moved from
  `commands.rs` into `clipboard.rs`, behind a clipboard chosen at run time.
  Copy and paste a value and a text range in the app, and paste into another
  app.
- **The control session.** `make check` now builds the `control` feature. Run
  `make control ARGS='--document examples/grap-demo.gid'`, then
  `tools/control screenshot` and a click.

## Later

- **Rust 1.100, due 12 November 2026.** Move the wrappers' `update` (and
  `sandbox-cargo resolve`) to stable Cargo, where
  `registry.global-min-publish-age` no longer needs `-Z min-publish-age`, and
  merge `sandbox-cargo`'s near-identical branches. `linux-cargo update` can
  change on Linux; check `sandbox-cargo`'s on a Mac. See
  [build security](build-security.md).
