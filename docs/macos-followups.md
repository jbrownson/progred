# macOS follow-ups

Changes from the October 2026 cloud sessions that were built and tested on
Linux only.
Check each on macOS, then delete it from this list; delete the file when it's
empty.

## Next

- **The clipboard.** The system clipboard's copy and paste moved from
  `commands.rs` into `clipboard.rs`, behind a clipboard chosen at run time.
  Copy and paste a value and a text range in the app, and paste into another
  app.
- **The 18–19 October cleanup's macOS-only code.** It was checked with
  `--target aarch64-apple-darwin` but not run: `native_menu.rs`, whose
  `Menu::new` takes the app's stack's projections and whose View menu's pane
  commands carry their side or direction; the `ns_view` helper through which
  `macos_window.rs` and `macos_surface.rs` read a window's AppKit view; and
  app.rs's macOS branches: `App::new`'s menu and window cascade,
  `create_window`, `activate`, `change_palette`, and theme changes. Open
  several windows, open and move panes from the View menu, switch a library's
  projection off and on from the Projections menu, and switch between Light,
  Dark and Follow System Appearance.

## Later

- **Rust 1.100, due 12 November 2026.** Move the wrappers' `update` (and
  `sandbox-cargo resolve`) to stable Cargo, where
  `registry.global-min-publish-age` no longer needs `-Z min-publish-age`, and
  merge `sandbox-cargo`'s near-identical branches. `linux-cargo update` can
  change on Linux; check `sandbox-cargo`'s on a Mac. See
  [build security](build-security.md).
