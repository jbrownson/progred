# macOS follow-ups

Changes from the October 2026 cloud sessions that were built and tested on
Linux only.
Check each on macOS, then delete it from this list; delete the file when it's
empty.

## Browser

- **`node tools/test-web-gpu.cjs`**, as the [browser notes](../web/README.md#diagnostics)
  describe. It needs Playwright with Chrome, which this machine doesn't have.
  `make run-web` with WebGPU draws the IoP tree correctly.

## Later

- **Rust 1.100, due 12 November 2026.** Move `sandbox-cargo update` and
  `resolve` to stable Cargo, where `registry.global-min-publish-age` no longer
  needs `-Z min-publish-age`, and merge their near-identical branches. See
  [build security](build-security.md).
