# macOS follow-ups

Changes from the October 2026 cloud sessions that were built and tested on
Linux only.
Check each on macOS, then delete it from this list; delete the file when it's
empty.

## Next

From the November 2026 cleanup pass (101 commits after `dbec690c`), built and
tested on Linux only; its macOS code was checked with
`--target aarch64-apple-darwin` but not run. In order:

- **The build sandbox.** `tools/sandbox-cargo` now takes its environment from
  `sandbox_env` in `tools/cargo-common.sh`, shared with `tools/linux-cargo`
  (`f757f1f5`), and `tools/rust-analyzer-proc-macro-srv` goes through
  `tools/rust-analyzer-cargo` (`7b4ce691`). Run `make check`, `make test`
  (1405 passing on Linux) and `make verify` (12 and 255 items); open the
  project in an editor with rust-analyzer and see that proc macros expand. If
  the sandbox fails, revert those two commits before anything else.
- **GPU tests.** `puri_vello::capture::Gpu::new` returns a `Result`
  (`6af6ad2c`), compile-checked only. Run `make test-gpu`, and `make
  test-web-gpu`, which is macOS-only.
- **Images.** Run `make images` and see that no checked-in image changes: the
  pass meant to change no drawing.
- **The app's macOS branches.** `Editor::active_window()` replaced the
  hand-written matches, among them the one that renames a document's window
  frame (`2e871e82`); `platform::drawn_menu()` replaced a constant
  (`b3054db9`); the frame builds its menu input only when it draws the menu
  (`2f92cc0c`); a lost surface is configured again instead of panicking
  (`d080be71`). In the app: open two windows, save an untitled document under
  a name and see the title and proxy icon follow, use the native menus and
  their shortcuts, including a library's and a projection's entries, sleep and
  wake the machine with a window open, and move a window between displays.
- **Speed on the Mac.** The pass was measured in instructions per frame on
  Linux (about 1 to 2% fewer on five canaries, 0.3% more on the IoP picture).
  The owner found a native demo in early November slower than remembered. Run
  `make perf ARGS=--editor` at `dbec690c` and at the head and compare wall
  times; then orbit a CAM preview and drag the forest's slider by hand.
- **Edit's New Library from Selection.** A new command in the native Edit
  menu, enabled only while a record no declared library draws is selected;
  checked with `--target aarch64-apple-darwin` but not run. Select a record,
  choose it, and see the library appear under the root's `libraries` with its
  template selected, then undo it.
- **To write: don't sync the native menu on every redraw.** `App::sync_menus`
  (app.rs) calls `native_menu.sync` with the editor's availability and toggles
  from five places, one of them each redraw. Keep the last pair synced and
  skip the call when it's unchanged; measure that it pays before keeping it.

- **The clipboard.** The system clipboard's copy and paste moved from
  `commands.rs` into `clipboard.rs`, behind a clipboard chosen at run time.
  Copy and paste a value and a text range in the app, and paste into another
  app.
- **A copy's picture.** A structural copy now also puts a PNG of what's drawn
  on the pasteboard, between the private format and the text, rendered on the
  window's GPU and written once the key is handled (`picture.rs`, `app.rs`'s
  `picture_copy`). Copy a list or a record, including one with a 3D preview,
  and paste it into an app that takes images, such as Notes or a chat, and
  into a plain-text editor; check the picture's size on a Retina display.
- **Help → Copy Version.** The native menu's new Help section was checked with
  `--target aarch64-apple-darwin` but not run. Choose it and paste into another
  app: it should give the window title's version, such as `pre-alpha 1a2b3c4d`.
- **The 18–19 October cleanup's macOS-only code.** It was checked with
  `--target aarch64-apple-darwin` but not run: `native_menu.rs`, whose
  `Menu::new` takes the app's stack's projections and whose View menu's pane
  commands carry their side or direction; the `ns_view` helper through which
  `macos_window.rs` and `macos_surface.rs` read a window's AppKit view; and
  app.rs's macOS branches: `App::new`'s menu and window cascade,
  `create_window`, `activate`, `change_palette`, and theme changes. Open
  several windows, open and move panes from the View menu, choose All, Plain
  and Raw and switch a library off and on from the Projections menu, and
  switch between Light, Dark and Follow System Appearance.

- **`make verify`.** The wrappers' `verify` command and
  `tools/verus-settings.sh` ran on Linux only. On a Mac, install the 1.99.1
  toolchain and run `make verify`: it should download the arm64 release, match
  its recorded digest, unpack it under the Seatbelt profile, and prove
  `gid/src/position.rs`. Its `rust_verify` finds the compiler's libraries only
  through `DYLD_FALLBACK_LIBRARY_PATH`, which the command sets inside the
  sandbox; see [verification](verification.md#running-it).

- **The Libraries menu.** The native bar's new Libraries section, between
  Projections and Window, was checked with `--target aarch64-apple-darwin`
  but not run: `native_menu.rs`'s `definition` and `Menu::new` take the loaded
  libraries' names beside the projections. Choose a library: a pane on the
  right shows its name over its cells; choosing it again selects that pane.

## Later

- **Rust 1.100, due 12 November 2026.** Move the wrappers' `update` (and
  `sandbox-cargo resolve`) to stable Cargo, where
  `registry.global-min-publish-age` no longer needs `-Z min-publish-age`, and
  merge `sandbox-cargo`'s near-identical branches. `linux-cargo update` can
  change on Linux; check `sandbox-cargo`'s on a Mac. See
  [build security](build-security.md).
