# macOS follow-ups

Changes from the October 2026 cloud sessions that were built and tested on
Linux only.
Check each on macOS, then delete it from this list; delete the file when it's
empty.

## Next

- **`sandbox-cargo` and `make check`.** `sandbox-cargo` now checks the OS
  before looking for rustup, and takes the pinned nightly for `update` and
  `resolve` from `tools/web-build-settings.sh`. `make check` now covers every
  target and the `cam-profile` and `layout-profile` features. Run `make check`
  and `./tools/sandbox-cargo update --dry-run`.

## Later

- **Rust 1.100, due 12 November 2026.** Move the wrappers' `update` (and
  `sandbox-cargo resolve`) to stable Cargo, where
  `registry.global-min-publish-age` no longer needs `-Z min-publish-age`, and
  merge `sandbox-cargo`'s near-identical branches. `linux-cargo update` can
  change on Linux; check `sandbox-cargo`'s on a Mac. See
  [build security](build-security.md).
