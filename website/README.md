# prog.red website

This is the local website foundation. It embeds the actual browser editor;
there is no separate demo implementation, framework, or hosting dependency.

## Open it

On macOS, double-click **Preview.command** in Finder, or run `make run-website`
from the repository root. It starts a loopback-only web server and leaves the
URL as the last line in the terminal. Open that link in whichever browser you
want. Leave the terminal running; Control+C stops the server. Website and lesson
edits need only a browser refresh. Successful requests are quiet; errors remain
visible with the URL repeated below them, so ordinary browsing doesn't bury it.

The local address is always <http://127.0.0.1:8081/>. Closing a browser tab does
not stop the server: reopen that link in Safari, Chrome, or another browser.
It doesn't launch a browser unless requested with `make run-website ARGS=--open`.
The server must remain running; after stopping it or restarting the computer,
start it again. There is no background service or automatic login startup.

Each launch first builds the browser editor with the repository's isolated
Cargo workflow, so Rust editor changes are always included. Cargo skips the
work when nothing changed (about a second), and `wasm-bindgen` reruns only
when the compiled editor is newer than its generated files. The very first
build can take a while. `--no-build` reuses the existing generated files
without checking for changes, building only if they're missing. Restart the
launcher to pick up editor changes; website files only need a browser refresh.

The browser build uses `nightly-2026-08-27` with `rust-src` and `llvm-tools`,
and `wasm-bindgen-cli` matching the locked `wasm-bindgen` version. It rebuilds
`std` with atomics and enables SIMD for the shared-memory browser build, inside
the normal Cargo sandbox. The browser must support WebAssembly SIMD.
One-time setup is described in [the browser host notes](../web/README.md).
No Node or package installation is needed for the site.

The header's GitHub link uses the standard Octicon and the site's light/dark
palette. A small local script requests the star count once from GitHub's public
API, without credentials, retries, or a third-party widget. Until it succeeds
(or if it fails), the icon remains an ordinary repository link. Switching themes
only changes CSS. The icon's MIT license is included in `public/octicons-LICENSE.txt`.

Cloudflare Web Analytics counts visits. `public/analytics.js` loads Cloudflare's
beacon only on prog.red and its subdomains, and only in a top-level page, so a
tutorial's embedded editors don't each count as a page view; the full editor
counts when opened on its own. The beacon sets no cookies and stores nothing in
the browser, and its token is public by design. In the Cloudflare dashboard,
prog.red's Web Analytics setting is manual installation: automatic setup would
inject the beacon into every embedded editor too.

Opening `public/index.html` directly is not supported: the WebAssembly module
and JavaScript imports need HTTP rather than a `file://` origin, and the threaded
editor needs the isolation headers provided by the local server.

`python3 website/preview.py` is the same launcher without the shell wrapper.
`--open` also opens the default
browser (`--no-open` explicitly retains the default link-only behavior), and
`--port 8082` requests another port. An occupied port is an explicit
error, not a silent change of address. Use `--port 0` to ask the operating system
for a free port when running an additional preview alongside this one.

## Work on it

- `public/index.html` is the home page: the pitch, a live forest whose projections
  and names you can switch off (`public/peel.js`), the key ideas, what works today, the three
  tutorials, and the Zulip links. Narrow screens get stills from `public/images/`
  instead of loading the editor.
  `make images` regenerates those stills, and the copies of the
  [README screenshots](../docs/images/README.md), after editor changes.
- `public/tutorial.html` is the exercise tutorial described below. `public/story.html` and
  `public/lab/peel.html` are the alternative tutorials the home page asks
  visitors to compare.
- `public/style.css`, `public/lessons.js`, and `public/appearance.js` are shared. Edit and refresh.
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

The exercise tutorial opens with a growing forest. A slider scrubs growth forward or backward;
the visible `drawing with controls` call connects it to a forest function with editable
tree count, growth rate, leaf color, and trunk color. Drawing and controls use existing
libraries; all forest geometry is ordinary Grap in the same document.
Eight exercises then build up the ideas in one reading column (a paragraph,
the editor, its steps, then an aside), each ending on the step that demonstrates
its idea: values edited directly (a planet record with a list), projections
(one calculation drawn three ways), what everything is made of (the planet in
the base projection), one value shared through cells, live calculations with two
puzzles, functions (including a call nested in a call), pictures as views of
code (including adding a call), and a three-tree finale combining them.
All use the real editor without its application menu, and explicitly select
`wheel=auto` so scrolling over an exercise scrolls its overflowing document or
completion popup when possible, and otherwise scrolls the website. The full-page
editor keeps its ordinary wheel scrolling and zooming.
Each iframe explicitly selects its libraries: name/text/blob plus number/f64
for the values, model, and cells exercises.
The calculations and functions exercises add Grap and absent to the numeric set.
The drawing lesson and the tree finale add control, color, and layout; the
drawing lesson credits Bret Victor's Inventing on Principle. The opening forest
adds controls, presentation, logic, list, and sequence to the drawing set,
without loading Fidget or toolpath libraries.
Calculations, functions, drawing, and the finale use `tutorial-slots`, listing three record-field
identities in display order. An entry-only projection stacks those fields without labels or insertion
gaps. An identity may carry a level: `:plain` draws that slot with only names,
text, and numbers read as themselves (calls stay records), and `:raw` draws it as
Raw does, without names. Several slots can show one shared cell at different levels. Deleting a value leaves its slot visible as the ordinary empty picker;
refilling it writes the same field. Nested values use the ordinary projection
and editing behavior. This is tutorial host configuration, not document syntax
or a library construct. Without the option, the record displays normally.
The opener stacks an ordinary `render` result above its editable call, without
panes. Trunks extend before the canopy expands, with deterministic variation
and staggered growth across trees. There is no animation clock, worker job,
or demo-specific runtime: the control value directly drives each drawing.
The scene explicitly sequences separate sky, ground, sun, and forest calls using
the control library's `all`. It evaluates each expression in order and retains
all results, including absents, so an incomplete inserted call does not suppress
later drawing. Inside `forest`, `for each` consumes a streaming `range` and the
forest calculates each tree's position and size. Fractional counts round up only
for iteration; spacing and growth still use the original count. Individual trees
do not take the forest's count. The sun follows an arc across the sky with the slider, independently
of the tree growth rate. The editable sky, ground, and sun colors are ordinary color
arguments too. `drawing with controls` is an ordinary Grap helper over `with controls`
and `draw`, keeping the canvas dimensions and callback wrapping out of the visible
scene. Its arguments are `controls` and `drawing`. The controls lambda receives
the stored UI state and an update callable. Here the state is just a float;
the absent library's `or default` supplies 0.65 when the state is absent. The slider receives
that value and the update callable as its change handler, and returns the
current value to the drawing lambda. No quoted state key is needed.
An optional `clouds {time, speed, color}` function is available through ordinary
cell completion but is not called by the initial scene. Its definition lives in
the root's `functions` list (outside the two tutorial display slots). Add its call
after `sky`, connect `time` to `parameters`, and use speed 1 for a gentle drift
of 60 logical pixels across the slider's full range. Speed 0 holds clouds still;
negative speeds reverse direction. This is ordinary Grap drawing, not a clock.
The inline drawing currently has an explicit 680 × 300 logical size.
The earlier machining fixture remains available as `lessons/shape.gid`.
It has no checklist or document-observation channel. Later lessons retain
individual checkmarks without aggregate completion counters.
The unused libraries are not constructed or offered by the completion picker;
list editing itself does not require Grap's list-operation library.
Document shortcuts remain available: Cmd on Mac, Ctrl elsewhere. The browser
host and tutorial share `web/platform.mjs`; the host supplies that convention to
the editor explicitly, including text editing, source links, and menu labels.
The full-page editor remains blank on startup
and retains its menus. The public site is deployed at <https://prog.red>.

The site defaults to light mode, with white editor surfaces distinct from the
warm cream page. The sun/moon switch selects light or dark mode for both the page and live editors
without reloading lessons or losing edits/checkmarks. The preference is stored
locally when browser storage is available; otherwise switching still works for
the current page. New and reset embeds receive the current theme. CSS variables
define the site colors; the editor's `display/widget/style/palette.rs` defines
semantic colors for each preset. Drawing colors belong to the document and are
not recolored by the theme.

The instructions check off when the actual document or selection satisfies
the step. Achievements stay checked through later edits and undo; Reset clears
only that exercise's checklist. Progress isn't saved or reported. The
predicates live in `public/lesson-progress.mjs`, not in the editor.

The values lesson edits a planet record: its name, its number of moons, and a
list of colors. Adding a color through a comma counts once the list grows.
Removing a color checks for a decrease from the previous observed list length;
erasing an item's text alone does not count. Restoring it is a separate step
that checks the list grows back to a previously observed state after a removal,
including undoing the whole text-erasing run. Inserting a different color does
not count. These checks observe results rather than keystrokes, and the small
list snapshots are discarded on restoration or Reset.

The projections lesson, right after values, answers "if a program isn't text,
what do you see?" Three slots reference one shared calculation cell, drawn fully
(`3 + 2 → 5`), plainly (`{evaluate: {function: +, left: 3, right: 2}}`), and
raw (identities and bytes). An edit counts in the view whose selection made it:
changing the number on top, then in the middle; the last step selects that
number in the raw view. Every slot view that isn't fully drawn carries a
caption naming what drew it, so the views explain themselves.

The model lesson (`model`) shows the planet again, fully drawn above and in the
base projection below, both referencing one cell. Its prose names the four
building blocks: records, lists, blobs, and cells. Renaming the planet and
adding a color count from the document; the last step counts a selection in the
base view inside the planet key's entry, which the editor also highlights in
the top view.

The cells lesson starts with two references to one numeric cell. It checks for
editing that shared definition, creating a different cell containing 11,
inserting another reference to the new cell, then editing its shared contents.
Checks use cell identities and the document's cell table, not matching displayed
numbers. Repeated independent numbers don't count as sharing. The instructions
use the ordinary `(` constructor and Cmd/Ctrl-click picking; creating a cell selects
the reference, so the user then clicks its empty contents to fill it.

The calculations lesson (`grap`) uses one unnamed numeric cell in two
calculations: addition and multiplication. The same cell also appears on its
own. Users edit a literal argument, changing only one result, then the shared
cell, changing both. Two puzzles follow: make the sum and product equal, then
make the product exactly 100. The checks compute those from the stored numbers;
the editor performs the actual evaluations and projects their read-only results.

The functions lesson starts with a named `scale` function, one parameter `x`,
and two evaluated calls. Its checks cover changing the first call's argument,
changing the function's multiplier, renaming the parameter, and nesting a call to
`scale` in the second call's argument. Emptying that argument leaves the declared
parameter as an ordinary empty picker, where typing `scale` offers the call.
Parameter uses and argument labels share the same cell identity; names don't
implement binding. Native interaction tests exercise the actual editing and
evaluation, including unchanged results after renaming and the nested result.

The drawing lesson uses an ordinary `dot(x)` function that calls `fill`, a `do`
program calling it twice, and a drawing of that same program. Users move one
circle by editing its call, enlarge both by editing the shared radius, add a third
call through the comma between the calls, then Cmd/Ctrl-click any circle to select
the `fill` call that produced it. Existing source tracing also links hover in
both directions. The drawing slot appears above the two source slots.

The finale reuses `forest.gid`: a `tree(x, height)` function called three times.
Its checks cover changing an original tree's height (matched by that tree's `x`,
so planting between trees doesn't count), recoloring the shared canopy paint, and
planting a fourth tree with both arguments. A new call opens its first argument;
its height is then its own empty picker. Headless interaction tests perform each
authoring step: the nested call, the third dot, and the fourth tree.

An embed is an ordinary editor URL:

```html
<iframe src="./editor/?document=../lessons/values.gid&menu=hidden&wheel=auto&threads=1&observe=values-0"
        title="An editable planet record" loading="lazy"></iframe>
```

The actual lesson URLs also include `libraries`, a comma-separated list of
existing library CellIds in their intended order. Omission retains the full
editor's default set; `libraries=` explicitly selects none. These are host
startup inputs, not hidden configuration in a lesson document. The shared
browser binary still contains the full editor's code.

`document` resolves relative to the editor URL. A failed fetch or parse displays
an error, not an empty substitute. `menu=hidden` removes the menu and its layout
space together with its command shortcuts (including Raw mode and examples).
Editing, navigation, and undo/redo remain independently available; this does not
restrict what data can be edited.
Browser key events run the installed editor handlers synchronously and cancel
the browser default only when handled. A capture listener bypasses Winit's
unconditional keyboard cancellation and prevents duplicate dispatch. The web
event-loop adapter and DOM callback share the same app; native routing is unchanged.
`wheel=auto` captures scrollable editor regions and otherwise leaves input to
the browser. `wheel=page` always leaves it to the browser; omission or `wheel=editor` keeps
editor scrolling/zooming. This is independent of menu visibility and embedding.
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
does not provide the header configuration this threaded build requires. The
Cloudflare configuration below supplies them without a service worker.

## Publishing on Cloudflare

This is a static website using Workers Static Assets, not a server-side Worker.
`wrangler.jsonc` points at `target/website`; `package.py` assembles that directory
from `public/`, the editor page and worker scripts, and wasm-bindgen's generated
modules. Source files, development diagnostics, and build tools aren't published.
`_headers` applies isolation headers to the page, embeds, WASM, and worker scripts.
Assets revalidate on each visit, and unknown URLs return 404, not the homepage.

To build a publishable directory locally on macOS:

```sh
make build-website
```

For a disposable Linux CI runner, `npm run build` in this directory invokes
`build-ci.sh`. It requires `CI=true` and x86_64 Linux, installs the pinned Rust
nightly and the official wasm-bindgen CLI release matching `Cargo.lock`, builds
the browser binary, runs the website tests, and packages the assets. The CLI
download is checked against the release's SHA-256 file. This path deliberately
uses ordinary Cargo, not Seatbelt; don't use it as a local sandbox bypass.
`tools/web-build-settings.sh` supplies the same Rust version, target, and threaded
WASM/SIMD flags to both build paths. No native editor code changes are required.

One-time Cloudflare setup, after the deployment changes are committed and the
chosen commit is pushed to a `website-live` branch on GitHub:

1. In Workers & Pages, choose **Connect GitHub** and authorize just
   `jbrownson/progred`.
2. Name the Worker **progred** (matching `wrangler.jsonc`).
3. Set the production branch to **website-live**, root directory to **website**,
   build command to **npm run build**, and deploy command to **npm run deploy**.
4. Leave non-production branch builds disabled. Cloudflare installs the locked
   npm dependencies and supplies `CI=true` and deployment authentication.
5. Check the generated HTTPS `workers.dev` address first. Then add `prog.red`
   as a custom domain; DNS setup is a separate step, not part of the build.

Do not connect the development branch as production just to get through the
wizard: **Settings → Build → Branch control** can select `website-live` if it
isn't offered during creation. Don't deploy until the production branch is set.

`try`, `iop`, `cad`, and `cam.prog.red` are proxied `AAAA 100::` DNS records,
each with a Redirect Rule (302) to the matching short path, so `_redirects`
decides what each one opens.

Publishing means advancing `website-live` to a tested commit, not copying files
or merging a second implementation of the website. An explicit
`git push origin HEAD:website-live` publishes the current commit (only when
intentionally requested); commits/pushes to `master` alone don't publish.
Don't force-push to undo a release: use Cloudflare's deployment rollback for an
immediate rollback, then fix/revert the source and publish a new commit.
No credentials belong in this repository.

The hosted Linux build and public site were verified on September 21, 2026.
The generated build directory and npm dependencies are ignored by Git.

Run the preview server and browser script checks without opening an editor
(the latter uses Node's built-in test runner, with no dependencies):

```sh
python3 -B -m unittest discover -s website -p 'test_*.py'
node --test website/test_*.cjs
```

The editor's tests follow every tutorial page's steps (`progred/src/tutorials`).
Each opens an embed with the document, libraries, and slots its page's iframe
names, does what the page says with pointer and key events, and must try each of
the page's numbered tasks in order, so changing a page's embed or tasks means
changing its test:

```sh
./tools/sandbox-cargo test -p progred --lib tutorials
```
