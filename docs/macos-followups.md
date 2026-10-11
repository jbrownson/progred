# macOS follow-ups

Changes from the October 2026 cloud sessions that were built and tested on
Linux only.
Check each on macOS, then delete it from this list; delete the file when it's
empty.

## Check in the app

- **Keys in the app.** Paste, delete, insert, replace, Select All, and arrow
  navigation moved from shell code into the frame's lowest-priority handler.
  Try arrows, Backspace and Delete, Enter, Command+V, and Command+A in `make run`.
- **Presentation and color.** Windows now present through `Queue::present`, and
  wgpu 30 changed Metal color-space handling. Check drawing, resizing, and
  colors in `make run`.
- **Text.** Parley 0.11 brings a newer shaping engine. The captures show no
  drift, but they lay text out with the bundled fonts; compare the app's text,
  which uses the system fonts.

## Browser

- **`node tools/test-web-gpu.cjs`**, as the [browser notes](../web/README.md#diagnostics)
  describe. It needs Playwright with Chrome, which this machine doesn't have.
  `make run-web` with WebGPU draws the IoP tree correctly.

## Later

- **Rust 1.100, due 12 November 2026.** Move `sandbox-cargo update` and
  `resolve` to stable Cargo, where `registry.global-min-publish-age` no longer
  needs `-Z min-publish-age`, and merge their near-identical branches. See
  [build security](build-security.md).
