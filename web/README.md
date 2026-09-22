# Progred in a browser

From the repository root:

```sh
make serve-web
```

Open `http://127.0.0.1:8080/editor/` on this machine.
The built-in documents are under the **Examples** menu.

For a small embedded editor, supply a text-bridge document URL and optionally
hide the menu: `/editor/?document=../lessons/values.gid&menu=hidden&threads=1`.
The document URL is relative to the editor page. Without those parameters the
editor still starts blank with its full menu and the default worker pool.
`wheel=page` leaves wheel input to browser scrolling rather than editor scrolling
or zooming. A passive capture listener keeps those events out of the editor,
without cancelling their browser default; the embed permits scroll chaining on
both its document root and body. Clicks, dragging, and keyboard input are unchanged.
Omitting the option (or `wheel=editor`) keeps normal editor wheel handling,
even when embedded or using a hidden menu. Unknown wheel modes fail explicitly.
The tutorial embeds select `wheel=page`; the full editor does not. This option
belongs entirely to the JS host, not the WASM entry point or native editor.
Hidden-menu embeds retain document shortcuts (including Undo/Redo), but not
application shortcuts or F10 menu navigation. The website owns the exercise
documents and reset buttons. An optional `tutorial-slots` parameter lists distinct
record-field CellIds in display order. The tutorial entry projection shows just
those contents in a fixed column, without labels or insertion gaps. Missing
fields remain ordinary editable empty slots; nested values retain normal editing.
Omitting this option keeps the standard document projection.
Canvas and window focus/blur events control the editor's active presentation.
Leaving an embed clears its selection, caret, completion query, and related
highlights. Document edits remain. Loading another lesson does not steal focus.
The WASM entry point is `start_editor(source?, show_menu?, on_change?, libraries?, tutorial_slots?, command_is_meta, theme?)`; malformed supplied
documents fail explicitly before starting the editor. See the
[website notes](../website/README.md) for independent embedded sessions.

`theme=light` or `theme=dark` selects an editor palette; otherwise the JS host
uses the website's saved preference, defaulting to light. A same-origin parent
can send `{type: "progred:theme", theme: "light" | "dark"}` to change it in place.
The host updates its loading surface and calls the WASM `set_theme` export,
which queues an ordinary full frame rebuild without resetting editor state.
The embed sends `{type: "progred:ready"}` after startup so its parent can resend
the current preference after lazy loading or a lesson reset.

The host supplies `command_is_meta` using `platform.mjs`: Command on Mac, Control
elsewhere. Editing, picking, source links, and drawn menu labels use this same
convention. The tutorial imports the same helper for its shortcut instructions.

`libraries` is a comma-separated list of built-in library CellIds, in projection
precedence order. These are the library identities, not their field tags or
display names. Only those libraries are constructed and contribute definitions,
projections, and completions. Omission loads the full default stack; an explicit
empty string loads no libraries (structural editing remains available). Unknown
or malformed IDs fail explicitly, without falling back to the full stack.
There is no automatic dependency loading: the embedding host chooses the set.
The website's record exercise loads name, text, blob, number, and f64; its list
exercise loads only name, text, and blob. They still use the same WASM binary
and worker setup, not separately stripped-down builds.

The optional callback receives a JSON string after the initial browser paint
and after document or selection changes. Hover, caret movement within the same
value, and ordinary repainting do not serialize the document. With no callback,
there is no observer or snapshot cost. Observations contain `document` using
GID's existing Serde representation, and `selection` (or null): `view` is
`"document"` or `{pane: path}`, `path` is the occurrence, `source_path` is the
optional document destination, and `stage` is `"value"`, `"pending"`, or
`"label"`. Paths use the existing path-library GID encoding; list positions
have session lifetime. This reports structural selection, not text caret/ranges.

`observe=<channel>` connects that callback to same-origin parent messages:
`{type: "progred:change", channel, state}`. Only explicitly observed embeds
send document-change messages. The website owns the checks,
achievement state, and reset behavior. This is a read-only host observation
boundary, not a tutorial feature in the editor.

Click a menu heading to open it; while a menu is open, moving over another
heading switches to it. Mouse and keyboard share one highlighted item.
F10 opens/closes the menu bar, arrows navigate, Home/End select the first/last
enabled item, and Enter/Space activates it. Escape, Tab, an outside click, or
losing focus dismisses the menu. The displayed Cmd/Ctrl shortcuts work with menus
open or closed. The native macOS menu remains separate.

The browser build uses a shared-memory coordinator for the existing CAM background
jobs. It requires a secure context (localhost or HTTPS) and cross-origin
isolation, and WebAssembly SIMD support. The local server supplies
`Cross-Origin-Opener-Policy: same-origin` and
`Cross-Origin-Embedder-Policy: require-corp` for the site, editor, and worker.
A plain file URL, ordinary `python -m http.server`, or LAN HTTP is not sufficient.
Unsupported hosts show an error instead of silently doing heavy work inline.

One-time build prerequisites:

```sh
rustup toolchain install nightly-2026-08-27 --profile minimal
rustup component add rust-src llvm-tools --toolchain nightly-2026-08-27
./tools/sandbox-cargo web-threaded-fetch
```

Also install `wasm-bindgen-cli` matching the lockfile. `make build-web` rebuilds
the standard library with atomics and enables SIMD under Seatbelt, then generates
`web/pkg`.
Native builds continue using stable Rust. The worker instantiates exactly the
same module with shared memory; ordinary Rust `Send` closures and results stay
in Rust, while JS transfers job pointers and wake notifications. The page alone
starts the editor and dispatches completion events. Fatal worker errors require
a page reload; they are not treated as completed computations.

The coordinator initializes a `wasm-bindgen-rayon` pool before accepting jobs.
It uses up to eight rendering workers, leaving one reported hardware thread
free when possible. Fidget uses its existing parallel VM mesher/rasterizer;
this is not Fidget's experimental GPU interpreter or a browser JIT. A per-page
BroadcastChannel carries progress/completion wakes from any pool thread to the
page; the Rust results remain in shared memory.

Presentation uses the same Vello/image/triangle compositor as native, through
WebGPU. Tool/path meshes stay GPU resources across camera changes; implicit
images upload when replaced, with the same depth and clipping behavior. If no
WebGPU adapter is available, the host retains the slower Canvas2D/CPU-mesh
fallback. Startup logs the selected presentation backend and worker count.

`website/Preview.command` builds and opens the whole website. See
[the website README](../website/README.md) and
[worker diagnostics](../docs/web-worker-experiment-2026-09-19.md).
See also the [performance comparison](../docs/browser-native-profile-2026-09-19.md).
The [SIMD comparison](../docs/browser-simd-profile-2026-09-19.md) covers the browser-only build flag.
