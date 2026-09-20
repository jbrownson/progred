# prog.red website

This is the local website foundation. It embeds the actual browser editor;
there is no separate demo implementation, framework, or hosting dependency.

## Open it

On macOS, double-click **Preview.command** in Finder. It builds the browser
editor using the repository's isolated Cargo workflow, starts a loopback-only
web server, and opens the website in your default browser. Leave its Terminal
window running; Control+C stops the server. The first build can take a while.

The browser build uses `nightly-2026-08-27` with `rust-src` and `llvm-tools`,
and `wasm-bindgen-cli` matching the locked `wasm-bindgen` version. It rebuilds
`std` with atomics and enables SIMD for the shared-memory browser build, inside
the normal Cargo sandbox. The browser must support WebAssembly SIMD.
One-time setup is described in [the browser host notes](../web/README.md).
No Node or package installation is needed for the site.

Opening `public/index.html` directly is not supported: the WebAssembly module
and JavaScript imports need HTTP rather than a `file://` origin.

After the first build, `python3 website/preview.py` from the repository root
starts the preview without rebuilding the editor. `--no-open` leaves the browser
closed, and `--port 8081` requests a particular port. By default the operating
system chooses an available port, so an existing editor preview can stay open.

## Work on it

- `public/index.html`, `public/style.css`, and `public/lessons.js` are the website. Edit and refresh.
- `public/lessons/*.gid` are the small, ordinary documents used by the exercises.
  They are fetched at startup, not compiled into the editor.
- `/editor/` serves the existing `web/` host and generated `web/pkg/` build.
- Editor changes require `make build-web` and a refresh.
- The server exposes only these two directories, not the repository or its Git data.
- Each iframe owns an independent editor session. Its Reset button reloads only
  that iframe, discarding that exercise's edits and history. Full-page editor links open a
  new session; they do not transfer the embedded document.
- Browser edits are currently in memory only. Do not author something you need
  to keep here yet; document import/export is a useful next step.

The page begins with five guided exercises: editing a text/number record,
inserting into a list, creating and sharing cells, live Grap calculations,
then editing a function and its calls.
All use the real editor without its application menu.
Each iframe explicitly selects its libraries: name/text/blob plus number/f64
for the values and cells exercises, and just name/text/blob for the list exercise.
The Grap and functions exercises add Grap and absent to the numeric set.
Their `tutorial-slots` embed option lists three record-field identities in display
order. An entry-only projection stacks those fields without labels or insertion
gaps. Deleting a value leaves its slot visible as the ordinary empty picker;
refilling it writes the same field. Nested values use the ordinary projection
and editing behavior. This is tutorial host configuration, not document syntax
or a library construct. Without the option, the record displays normally.
The unused libraries are not constructed or offered by the completion picker;
list editing itself does not require Grap's list-operation library.
Document shortcuts, including Ctrl+Z / Ctrl+Shift+Z, remain available (also on
Mac, matching the browser host). The full-page editor remains blank on startup
and retains its menus. Nothing is deployed, and `prog.red`'s DNS is unchanged.

The instructions check off when the actual document or selection satisfies
the step. Achievements stay checked through later edits and undo; Reset clears
only that exercise's checklist. There is no persistence or analytics. The
predicates live in `public/lesson-progress.mjs`, not in the editor.
The fourth list step, removing an item, checks for a decrease from the previous observed
list length, including four items back to three after the insertion exercise.
Erasing an item's text alone does not count. It checks the outcome, so undoing
an insertion counts as removal too. Like the other steps, it counts toward
completion and stays checked through Undo until Reset.
Restoring the removed item is a separate fifth step. It checks that the list
grows back to a previously observed state after a removal, including undoing
the whole text-erasing run. Merely inserting a different item does not count.
These checks observe results rather than keystrokes: manually recreating that
same list also counts. The small list snapshots are discarded on restoration
or Reset.

The cells lesson starts with two references to one numeric cell. It checks for
editing that shared definition, creating a different cell containing 11,
inserting another reference to the new cell, then editing its shared contents.
Checks use cell identities and the document's cell table, not matching displayed
numbers. Repeated independent numbers don't count as sharing. The instructions
use the ordinary `(` constructor and Ctrl-click picking; creating a cell selects
the reference, so the user then clicks its empty contents to fill it.

The Grap lesson uses one unnamed numeric cell in two calculations: addition
and multiplication. The same cell also appears on its own. Users first edit a
literal argument, changing only one result, then edit the shared cell through
any reference, changing both. Its checks inspect the stored calls, shared cell
identity, and numeric contents; the website does not evaluate Grap. The editor
performs the actual evaluations and projects their read-only results. Native
interaction tests cover both kinds of edits. Named numbers and bindings are
not introduced here.

The functions lesson starts with a named `scale` function, one parameter `x`,
and two evaluated calls. Its three checks cover changing one argument, changing
the function's multiplier, and renaming the parameter to `amount`. Parameter
uses and argument labels share the same cell identity; names don't implement
binding. This uses ordinary Grap lambdas and calls, not a lesson-specific FFI.
The checklist checks the stored recipe, argument values, and parameter name;
native interaction tests exercise the actual editing and evaluation, including
unchanged results after renaming. Constructing a function from scratch is left
for a later exercise.

An embed is an ordinary editor URL:

```html
<iframe src="./editor/?document=../lessons/values.gid&menu=hidden&threads=1&observe=values-0"
        title="Editable greeting and count" loading="lazy"></iframe>
```

The actual lesson URLs also include `libraries`, a comma-separated list of
existing library CellIds in their intended order. Omission retains the full
editor's default set; `libraries=` explicitly selects none. These are host
startup inputs, not hidden configuration in a lesson document. The shared
browser binary still contains the full editor's code.

`document` resolves relative to the editor URL. A failed fetch or parse displays
an error, not an empty substitute. `menu=hidden` removes the menu and its layout
space, plus application shortcuts; it does not restrict what data can be edited.
`threads=1` gives each small exercise one rendering worker rather than a full
CAM pool. The independent iframe isolates its WASM instance, event loop, focus,
history, and worker lifetime. Lazy loading delays startup for distant examples.

`observe` opts into the browser host's document/selection notifications and
supplies a channel echoed in each message. The page checks the same-origin
sender, iframe, and channel; reset advances the channel so queued notifications
from the old editor cannot complete the new checklist. The full editor does
not subscribe, serialize state, or send these messages.

CAM stock meshing and implicit rendering use a shared-memory coordinator and
Rayon worker pool (up to eight rendering workers). The editor starts only after
the pool is ready; there is no synchronous computation fallback.
The server supplies the COOP/COEP headers required for cross-origin isolation
on both the website and embedded editor. Existing preview servers need to be
restarted after this change, not just refreshed. Projection and toolpath generation
remain on the page thread. Presentation uses the native compositor via WebGPU,
including GPU mesh drawing. Canvas2D/CPU mesh drawing remains a fallback when
WebGPU is unavailable.

For deployment, use HTTPS and configure those same headers. GitHub Pages alone
does not provide the header configuration this threaded build requires; no
service-worker workaround or deployment is configured here.

Run the preview server and embed-host checks without opening an editor
(the latter uses Node's built-in test runner, with no dependencies):

```sh
python3 -B -m unittest discover -s website -p 'test_*.py'
node --test website/test_embed.cjs
```
