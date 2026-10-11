# macOS follow-ups

Changes from the October 2026 cloud sessions that were built and tested on
Linux only.
Check each on macOS, then delete it from this list; delete the file when it's
empty.

## Next

- **`sandbox-cargo` and `make check`.** `sandbox-cargo` now shares its
  commands with `linux-cargo` through `tools/cargo-common.sh`, checks the OS
  before looking for rustup, takes the pinned nightly from
  `tools/web-build-settings.sh`, and fetches missing crates before a build as
  `make fetch` does. `make check` now covers every target and the
  `cam-profile` and `layout-profile` features, and the browser diagnostics
  share their preview-server setup in `tools/web-diagnostics.cjs`. Run
  `make check`, `make test`, `make build-web`, `make test-web-gpu` and
  `./tools/sandbox-cargo update --dry-run`.

## Later

- **Rust 1.100, due 12 November 2026.** Move the wrappers' `update` (and
  `sandbox-cargo resolve`) to stable Cargo, where
  `registry.global-min-publish-age` no longer needs `-Z min-publish-age`, and
  merge `sandbox-cargo`'s near-identical branches. `linux-cargo update` can
  change on Linux; check `sandbox-cargo`'s on a Mac. See
  [build security](build-security.md).
