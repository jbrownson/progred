# Deferred work

Open questions and next steps that aren't scheduled. The other docs describe
what exists; proposals belong here, and what must happen before a release is in
the [release checklist](release-checklist.md).

## Active insertion hidden by a specialized projection

A specialized projection can omit the structure where an insertion is pending.
The general interaction and presentation policy needs dedicated design work.
Do not hide a fallback policy in reusable widgets or infer it by inspecting a
render tree. This was explicitly set aside for another day.

The shared list/record combinators preserve immediate pending children. Compact
Grap lambdas, cases, binding clauses, and control/operator calls now decline when
they cannot show an active insertion. That fixes those specific projections;
it is not a general fallback policy for arbitrary facets such as line controls.

## Discoverability of expression definitions

Keep Grap expression references shallow, including the source shown by
`{evaluate: expression}`. The toolpath example places an ordinary reference to
the ball-tool cell immediately before its evaluation so the normal projection
exposes its editable contents. Both occurrences reference the same definition;
neither is a special declaration. Revisit navigation or expansion affordances
if finding these definitions becomes a recurring problem, rather than changing
`evaluate`'s projection now.

## Selection destinations and history

Plain selections now work with line controls' missing-state defaults, so paste,
deletion, and undo do not need editor-initialization hooks. Destination policies
may still merit customization: deletion chooses a surviving sibling or parent;
undo restores an edge location, not its prior caret or pending query. Preserving
those states is a separate UX decision, not necessary to make restored selections
usable. Never resurrect active drags or IME composition from history.

Line-editor conversion runs after every accepted event, including caret
movement; equal results already skip document writes. Avoiding unnecessary
conversion is a separate optimization. Projections must not rely on that
optimization to keep a missing value absent: a direct line control offers its
spelling for write-through. Lambda names instead use the completion picker,
which stages input until commit.

## Duplicate-definition inspection

Normal lookup selects the document definition, otherwise the first library
definition in load order. Duplicate definitions are tolerated, not an intended
composition mechanism. Later we may show an indicator and an inspector for the
other sources; no such UI is implemented now. Explicit source-qualified paths
remain available internally.

## Editor-authored libraries

A document can declare libraries that draw its own records (the root's
`libraries` field; see projections.md). What remains for a complete
editor-authored library: completion offers, so a library can construct its
values; library-scoped definitions and names, beyond ordinary document cells;
and loading a library defined in one document into another, as an explicit
editor operation with its configuration and definitions reachable from root.
The proposed descriptor lists cell references, not a second record duplicating
their definitions; loading needs that descriptor and a source that can resolve
the listed cells, which a document can provide without becoming part of the
library's semantic type.

## Quotes that fill bound variables

A quote fills only its `unquote` holes; a bare reference in a template stays
data. The preferred direction is for a quote to also fill every variable bound
around it, keeping `unquote` for computed expressions, so a template reads as
data with names: `"{circle: {x: x, y: 50}}`. It waits on two idioms that rely on
bare references staying data, both workarounds worth removing first:

- The CAM example writes deferred passes as closures in their stored form,
  `"{closure: {params, body, environment}}`, whose `body` refers to variables
  that `environment` captures through `unquote`. A lambda would capture its
  whole lexical environment. The original reason, not retaining recursive
  list-mapping accumulators, went away with scoped tree emission. Lambdas that
  capture only the variables their body uses would let these be ordinary lambdas.
- Keyed controls (`slider`, `radio`, `tree range`, `tree cursor`, and
  `tree program cursor`) take their state key as an evaluated argument, so
  programs quote a cell to pass it. The CAM's `control key: "preview mode` reuses
  the binder of the variable that holds the radio's value, and that cell is also
  a record key and a pattern binder in the view. The forest's controls receive
  their state and an update callable and need no key; giving `radio` and
  `tree program cursor` that form and retiring the keyed capabilities removes
  the idiom.

An audit of every document found 25 bare references to bound variables inside
templates, all in `examples/toolpaths.gid` and `website/public/lessons/shape.gid`,
which shares its code. Of the 198 holes across all documents, 122 are plain
variables and 76 are computed expressions, so `unquote` stays common in either
design. Independently of the semantics, the picker could offer in-scope variables
inside a template and insert them as holes.

## Address lifetime

Paths and source traces already preserve definition source. A future identity
review should specify what survives list moves, deletion/undo, definition
replacement, and transient-root changes. A cell has meaning in a resolution
context; neither a path nor a cell-relative trace is a global address.
Keep occurrence identity, cell identity, definition source, and view identity
distinct while answering those questions.

## Saving list positions

List elements have fractional positions, minted when a document loads and
dropped when it saves, so a position means nothing outside its session. A page
or a script can't name the second call in a list by path across a reload, which
is why the tutorial's checks compare selected values instead. Saving the
positions fixes that, and a CRDT would need them anyway. Reading could still
accept a plain `[1, 2, 3]` and mint positions for it, while saving always writes
them, even if the text gets noisier.

## Completion ranking

Revisit how library-provided offers rank alongside built-in constructors and
interpretations of the query. Grap's slots list the libraries' offers together,
so `new lambda` sits among them in library order rather than with the editor's
constructors. Providers cannot express that it belongs with other constructors.
Design that distinction through the completion interface rather than
special-casing lambda or particular libraries. This is deferred UX design, not a
priority for the current cleanup.

## Completion after the strip-back

Completion was stripped back to a deliberately simple rule: the projection
drawing a hole supplies its offers, and a hole without a projection's offers is
Raw's, which offers the editor's constructors and what's typed, read as a value.
A document's root leads with the libraries' starting points. The point is to
confront what completion really needs and build back up from there. What was
removed, so it isn't lost:

- **The `…` expansion.** A picker opened on a narrow list, from its projection or
  from a library, and `…` (or Down past the last row) added everything after it.
  That repeated offers, and made Grap copy the editor's constructors to have
  them in its list. Some way past a projection's list will probably be needed
  again, designed without copying offers.
- **Libraries' focused answers.** A library could answer "what belongs here?"
  for a hole it recognized from the path, and that answer replaced everything
  until expanded:
  - Fidget offered per-slot lists. A scene's entries and a shape operation's
    operands (left, right, operand, field) got sphere and circle calls, the
    shape operations (union, difference, intersection, translate, sum, subtract,
    multiply, divide, min, max, negate, abs, sqrt, square, sin, cos) and the X,
    Y and Z axes, then f32 numbers. A radius, delta or bound got only f32
    numbers; a width or height f64; a mesh depth u64; an axis X, Y or Z. Naming a
    field on a shape offered its missing parameters, and an inert operation's
    record offered the shape markers and axis as labels. Grap calls to shapes
    got the same through their arguments, and a new scene's first hole offered
    a sphere call.
  - The name library offered the query as text for any value under a `name`
    key. Now the name library's partial draws that hole where a projection
    passes it.
  - Libraries offered at the root: the new-scene and new-program templates (now
    their starting points, leading the root's hole) and root field labels:
    `grap`, `fidget` (also found by "sdf"), `panes` and `libraries`.
  - Grap's slots deferred to any library's focused answer before their own list.
- **The arithmetic rule.** In an operation's `left`, `right` or `operand`, and in
  its operator, the offers of the library defining the operation came first and
  hid other libraries' offers of the same spelling, so 3 in an f64 sum offered
  only f64's 3. The picker found the operation by looking one step up the path.
  A better form may be the operation's projection telling its operands' holes
  what to offer.
- **Bracket keys everywhere.** `(`, `[` and `{` made a new cell, list or record
  in every picker. They are now keys of Raw's holes alone. Elsewhere a delimiter
  is typed as text, which finds its constructor where the hole offers one, as
  Grap's slots do; a projection can't yet give its holes keys of their own.
- **Reaching any cell from a Grap slot.** Grap's list has bindings and functions
  but not other cells, such as a named constant.
- **Existing cells in Raw's holes.** Raw offered every cell by name, the
  document's and every library's (about 650 with the built-in libraries), with
  its source beside it, and a field's label picker offered existing cells as
  labels. It now offers none: an existing cell is reused by pointing at it
  (Cmd+click) or pasting it, and typing makes something new. Rebuilding that
  list every frame was also what made scrolling the picker hitch. A separate
  search by name could come back if pointing and pasting aren't enough.
- **The libraries' other offers in Raw's holes.** A new color, a new lambda, and
  calls to functions by name were offered in any Raw hole. Now only projections
  that ask the libraries for them offer them, as Grap's slots do.
- **Typed values for an empty query.** An empty query offered zero in each
  numeric representation, the empty string, and a new label with no name. Now
  it offers none of them; the empty string takes a `"`. A hole that expects a
  number or text, drawn by its own projection, could offer one for an empty
  query again.

Directions this surfaced:

- A record combinator taking a spec: expected fields in order, how each field's
  value is drawn and what its hole offers, missing expected fields shown as
  holes, then any other fields. Its "add a field" picker offers the missing
  expected fields. Lists already take a child projection, which can draw an
  element's hole with its own offers.
- Telling a partial which field it is drawn under, so a `name` field could reach
  the name library without each parent passing its partial. Handlers could
  later register for such patterns centrally, for efficiency.
- A type system. Knowing what fits an argument, such as a number expression for
  sphere's radius, needs types, since any expression can go there; how pressing
  that is is part of what this should show.
- Metadata on field definitions, such as types or projections, later.
- A projection's own offers and everything else, shown together without copies:
  perhaps in sections the reader can tell apart, or with the editor's built-in
  offers marked as wanted or not by the projection's offers.

## Starting points

A document's empty root offers the libraries' starting points, then Raw's
offers. There should be one for every direction a person might want to go, each
landing on something that works. Today's two, Grap's new program and Fidget's
new scene, each lead somewhere awkward; Grap's opens with a missing-cell error
in its pane until the program is written (that error's hole does take the
program now). Grap's might better start a journal, a notebook in the style of a
REPL; see [Notebook and REPL](#notebook-and-repl).

## Workspace documents

A document whose root has a `panes` field is a workspace, and the editor
should swallow it: show its panes, and draw only the main value in the
document view rather than the whole record. Discussed in October 2026, not yet
decided or started:

- **The root's shape.** `{<main>: value, panes: {left, right}, libraries}`.
  "primary" was rejected as the main field's name; `center` and `main` are the
  candidates. The `panes` structure could go either way; keep `libraries` as it
  is for now. Other roots stay valid documents.
- **Editing it by hand.** The Plain view shows the record with names. A "show
  panes" command could open the panes declaration in a pane.
- **Panes that show a document path,** not only a computed value.
- **Recursion.** A workspace in a pane of a workspace, which suggests making the
  pane layout a projection.

A suggested order: the root shape with the editor hiding the panes, converting
the examples; then path panes; then recursion.

## Code size and structure

An audit on 16 October 2026 ([the evidence](history/codebase-audit-2026-10-16.md))
found almost no dead code: 60% of progred's 94k lines are tests, much of them
inside source files, and 93% of its production code is one dependency cycle.
Its first recommendation is done: inline test modules now live in their own
files. projection/tests is still under projection/. It holds 20.8k lines that
mostly test the frame and the whole editor. The six private items it reaches
point at misplaced code, not at internals the tests should see. A follow-up
[crate split analysis](history/crate-split-2026-10-16.md) maps the coupling
that blocks a split and the steps that break it. The rest, most worthwhile
first:

1. Keep docs/history out of default searches, and point agents to the doc
   sections they need.
2. Share one `display::Env` test double and the common evaluate, apply and
   projection-input test helpers, now copied 19 to 29 times.
3. Take the crate split analysis's first four steps, each of which pays off on
   its own:
   - alias the concrete `<Editor, Hovered>` pair, written out 332 times;
   - let the model own the vocabularies and codecs it interprets, as grap owns
     f64's convention;
   - back `editing::Access` with the model and the libraries rather than
     `&mut Editor`, and let the shell set the window title after dispatch;
   - move the hover types, `projection::Cx` and the completion helpers down,
     and route calls into the projection runtime through `Project`.
4. Fix what projection/tests reaches for: 14 calls to six private items. Then
   move its frame- and editor-level suites out of projection/, leaving only the
   runtime's own tests, with nothing made visible just for tests.
   - The completion card, its placement and the selection-ring drawing
     (`completion_card`, `completion_placement`, `primary_highlight`) are
     display code filed in the projection runtime. The tests build the card
     with stand-in worlds such as `usize`.
   - `projection_target` builds display's `ProjectionTarget` from `Cx`, and
     moves down with it.
   - The frame harness calls `prepare_project` to time the projection's two
     phases, building the choice graph and resolving it to a width.
     `project()` fuses the two; expose both.
   - `Projection::without_entry` serves one profiling test.
5. Give the six things called "source" distinct names.
6. Give the occurrence-to-location mapping one owning module, and pass an
   opaque handle through display and the libraries instead of `Location` and
   `Conject`.
7. Split the CAM geometry into its own crate: small but easy.
8. Split progred into three crates, which would also keep the layering honest:
   - core: the model and display;
   - the libraries;
   - top: the projection runtime, the shell and the app.

   Do this only after item 3, a `cargo build --timings` baseline, and a `Host`
   trait that makes display and the libraries generic over the editor. The
   saving is in lines, not measured time, and `make test` could get slower for
   edits to core.

## Reporting completion failures

Offer construction decides applicability; activating an offered completion
consumes the input. Its Grap continuation can still fail at runtime. For now,
unsuccessful preparation installs neither the insertion nor its staged
selection/annotation changes. Declining after an effect is now an evaluator
contract error and prints to stderr; other completion failures remain silent.

Later, design an editor-owned way to surface these failures, such as alerts or
notifications, using the failure details in GID values. Do not add that machinery
now. Explicit decline is distinct from ordinary absent results, which are
definitive values and retain their effects. The eventual reporting policy must
not assume every absent warrants an alert.

## General computation reuse

The [computation graph](incremental.md) serves CAM geometry; canvas programs
still record once per visible frame, and other projections aren't converted.
Grap path generation is synchronous, and there are no Grap-language memo or
async boundaries, user-facing quality controls, or durability tiers. Future
readers of ordered contributor sets must observe that set, not just a selected
definition. Do not add an ad-hoc cache or event-specific relevance checks
alongside this mechanism. Incremental lambda calculus and incremental Fidget
algorithms are separate research.

The [2026-09-04 tree profile](history/tree-profile-2026-09-04.md) records measurements
from that implementation; timings are historical observations, not guarantees.

### Lowered results across evaluator boundaries

Public evaluation and application return owned `RuntimeValue`s, and the
projection, presentation, controls, drawing, layout, toolpath, and tree chains
retain runtime callbacks and results. Still GID-oriented, by explicit adapters:

- data-oriented projections and native functions, which can materialize runtime
  containers a later runtime-aware projection would have handled;
- domain and other legacy projections not yet migrated;
- individual `match` patterns, which adapt to the GID matcher, and
  repeated-binder equality, which compares materialized data;
- the narrow path, paint, and drawing-command parsers, and the viewport
  preparation memo's input;
- the 3D model, playback, and appearance decoders and render-failure outcomes;
- radio options and the older list-based range-control inputs.

Explicitly expanding a native closure's representation materializes its body
and environment, and document edits and copying are intentional GID boundaries.
Do not preserve a creator's dynamic stack as a caller. Keeping values lowered is
an efficiency choice, not the only way to preserve source annotations: closure
code origins round-trip as explicit GID metadata.

The [performance log](history/performance-log-2026-09.md#runtime-callback-handoffs--2026-09-23)
records the migration's measurements. Profile remaining conversions before
broadening it; keeping runtime values is not automatically faster. Exclude
compilation and give each compared source tree its own build output, since
sharing a target directory between archived copies reused incompatible artifacts.

`with controls` still returns a declaration interpreted by its partial. Direct
widget emission remains separate, deferred work. Profile uncached construction
as well as memo hits before changing that boundary.

### Fidget refinement and sharp edges

When we next examine Fidget internals for Progred's needs, revisit the
[analytic tool-sweep experiment](history/tool-sweep-experiment-2026-09-15.md). Its point signs pass
the sampled checks, but meshing yields invalid vertices and is slower than the
working approximation. Investigate stable gradients and interval behavior then,
alongside incremental/progressive computation; do not turn the current tool
profile work into a general analytic-sweep research project. This is not yet an
identified upstream bug. Tool geometry no longer carries approximation tolerance.

Future upstream investigation may cover reuse across progressive resolutions or
model edits, and more accurate surface-hit/normal evaluation at sharp edges.
Current raster refinement recomputes each level; finer depth sampling reduces
both missed thin intersections and wrong-face normals but does not eliminate
them. See the [tool-rim diagnostics](history/performance-log-2026-09.md#implicit-stock-quality-and-cancellation-investigation--2026-09-14).
Standalone implicit previews cancel even their first stage on new input. Revisit
finish-coarse scheduling only if a consumer without a usable fallback, such as
Command+3's retained mesh, needs it; obsolete results must never become the
current computation's result. Skipping intermediate meshes during rapid geometry
edits is another possible policy, with the tradeoff of an older mesh when the
user next orbits.

## Decoding PNGs once while visible

The [png library](../progred/src/libraries/png.rs) decodes each visible image
every frame. On the Linux container on 2026-10-12, editor screenshots rendered
through Vello took about 4–5 ms to decode at 1280 × 800 (123 KiB) and 13–14 ms
at 2560 × 1600 (286 KiB); reading only the header took about 2 µs. Each decode
also makes a new image, which the compositor uploads again, since it reuses
uploads by image blob identity. Adding the library didn't measurably change the
perf canaries. PNGs are mostly for sending over the control socket for now, so
this waits until they're shown on screen routinely, for example recorded
sessions viewed as storyboards.

When it's needed, follow the text-shaping cache (`puri::text::TextCache`), not
`incremental`. Decoding is a pure function of the bytes, so a memo keyed by them
can only be unused, never wrong. The editor would own it beside `text_cache`,
sweep it at the same point in `prepare_frame`, and reach the png widget through
the widget build context. A hit returns the same image, so its upload is reused
too, and one picture shown in two places decodes once. Keying by content hashes
the bytes each frame, which is small next to decoding; keying by the blob's
shared allocation, while holding a reference to it, avoids even that but needs
an accessor in `gid`. The projection would then decide from the header alone,
so a valid header with a corrupt body draws a placeholder instead of declining
to the record view. This would be a second exception to reusing work across
frames only through `incremental`; when it lands, AGENTS.md should name the
pattern it allows: caller-owned, swept memos of pure functions.

## Report Vello image-atlas flashing

Prepare an upstream bug report for the reproducible Vello 0.9.0 image omissions
documented in the [wide-preview investigation](history/graphics-memory-2026-09-16.md#wide-preview-flashing-follow-up-2026-09-18).
Repeated 4200 × 2800 image replacements drop every third image while 4000 × 2800
replacements work. The atlas protects obsolete recent images and silently omits
the current image when allocation fails. Reusing a registered texture works at
fixed size, but unregistering/replacing it during resizing reproduces the failure.

Before filing, check for an existing upstream issue and reproduce against the
then-current release with a minimal standalone example, including version,
backend, physical image dimensions, and expected/actual pixel readbacks. No
report has been filed. The owner prefers avoiding local Vello patches. The
[separate compositor](history/vello-compositor-experiment-2026-09-18.md) now bypasses the
atlas for native draw-image operations, with per-window upload ownership and
headless fidelity, resize, lifetime, and performance checks. Image brushes still
use Vello's atlas.

## CAM preview navigation and execution grouping

Explore an execution hierarchy distinct from the document's authoring outline.
The proposed meaning of an operation is a boundary where a person or robot
intervenes: flipping the part, re-probing, or similar work. Within it, indexed
orientations, tool uses, and logical groups of cuts are possible levels; their
ordering and names are not settled. Reusable tools and orientations need not
be owned by that tree: execution occurrences can reference the same definitions
in several places.

The [notched range controls](controls.md) follow ordinary nested program lists
and impose no such taxonomy. Producers that rebuild lists can change positions
the controls store; preserving that correspondence through Grap list
construction is separate work, not solved by inventing identities in the widget.

## Keyboard navigation and list reordering

Ordinary arrows now use [layout-derived navigation](navigation.md). Placeholder
Tab navigation, grow-selection commands, directional history, and moving a list
item with a modifier-plus-arrow shortcut remain separate decisions. The range
controls also remain pointer/touch-only.

## Inline lists with block children

An ordinary list's inline row uses its children's preferred forms, which can be
multiline. A possible standard-projection rule is to reserve inline sequences
for inline children, while allowing delimiters to enclose a block and explicit
layouts to put blocks side by side.

## CAM next steps

The toolpath example finishes indents and chamfers. Roughing, variable
orientation, collision checking for the non-cutting profiles, explicit links
between passes, and machine/postprocessor output are ahead. Render-quality
controls can use the controls library; path-generation tolerance belongs to the
program and is distinct from mesh/raster resolution. Upcoming-path windows and
transparency are also open.

## CAM machine-axis alignment

Revisit when generating G-code: prefer indexed orientations that let an
individual knurling pass use only two linear axes, where the actual compensated
trajectory and machine kinematics allow it. Surface-contact diagonals are
planar, but normal-offset ball-center trajectories need not be. A rotated
programming coordinate system alone does not imply fewer physical axes move.
Do not flatten trajectories or alter geometry in the postprocessor without an
explicit accuracy policy. Setup-up and pull/climb ordering are already explicit
in the Grap example; machine rotary solutions and fixture clearance are not.

## Peeking into folds

Clicking a fold's `…` opens it with the whole cell selected, so Space closes it
again. A look inside needn't change the layout at all: hovering the `…` could
show the contents in a popover, with Space or a click to open it for good, or
pressing and holding the `…` could open it until release.

## Hover traversal order

Revisit when the document/widget mix includes more expensive overlapping hover
targets, especially canvases covered by opaque popups or other content. Current
placement runs probes in painting order; foreground claims replace background
claims, but cannot avoid the work of background probes already run.

Compare this streaming pass with retaining probes and querying topmost-first,
where a direct hit or occlusion can skip probes underneath. Measure skipped
hit-test work against the extra allocations, retained captures, and traversal;
include ordinary source documents as well as overlap-heavy cases. Preserve
direct-over-extended claim priority, clipping, and nested-floater ordering, and
check identical hover, paint, and dispatch results. This is a deferred experiment,
not a decision to reverse traversal or add caching.

## Text input beyond plain typing

Native macOS IME is enabled while a text field is edited; dead keys and
interrupted composition were checked by hand on 2026-09-29. Left open, to take
up only when text entry becomes a focus:

- The macOS emoji and symbols picker inserts nothing. winit 0.30 forwards an
  `insertText` only while composition text is marked, and the picker inserts
  directly; opening it may also read as focus loss, which clears the edit.
  Needs a winit change first.
- The browser host enables no IME. Its keys arrive through the page's own
  keyboard bridge, so composition would need browser composition events.
- CJK composition and candidate placement, Linux IME, and AltGr characters in
  Windows browsers are implemented but unverified.

## Tutorial hints

The tutorial shows a hint when someone strays from a step in a way we can
predict, read from the same state as its checks: planting a tree without a
height. Review it, and more candidates from the newcomer runs, once the tutorial
settles:

- repeated plain clicks on a picture, which could point out holding Ctrl;
- dragging a number without Ctrl, which selects its text instead.

Both need the editor to tell the page about gestures, not just the selection.

## Platform work

### Opening documents from the macOS shell

The sandboxed app opens documents only through File > Open. A
sandbox-authorized shell open needs Launch Services to deliver
`application:openURLs:` rather than passing the path in `argv`, but Winit 0.30
owns the application delegate and panics if it is replaced
([#4015](https://github.com/rust-windowing/winit/issues/4015),
[#4458](https://github.com/rust-windowing/winit/issues/4458)). Winit 0.31
removes its custom macOS delegate. Revisit when 0.31 is stable or an upgrade is
otherwise useful; don't patch the Objective-C runtime around 0.30 for this.

### One sandbox for both platforms

The build sandboxes are two descriptions of one boundary: a Seatbelt profile on
macOS (`tools/cargo-sandbox.sb`) and bubblewrap's options on Linux
(`tools/linux-cargo`). Anthropic's
[sandbox runtime](https://github.com/anthropic-experimental/sandbox-runtime)
(`srt`), which Claude Code uses, drives both from one configuration, and its
proxy could narrow fetches from the whole network to the registry's domains. It
calls itself a beta research preview whose configuration may change, and it
would put Node on every build's path. Revisit when it's stable.

### Additional ports

The iPad feasibility host was removed while macOS/Linux mature; its notes are in
[history](history/ipad-host.md). Older language-extension ideas remain
uncommitted possibilities in the historical notes.

A visionOS version is interesting specifically for CAD/CAM: the same GID/Grap
model could project into spatial geometry while ordinary windows retain source,
parameters, and other editor projections. This should be treated as a spatial
projection/backend design, not merely the two-dimensional editor placed in a
headset.

Questions for that spike include the RealityKit/Metal boundary, spatial
selection and manipulation, and how existing source-to-output provenance maps
onto three-dimensional geometry. None requires changing the GID or Grap model
in advance.

## Directions

Larger directions from brainstorming in October 2026, written down so they
aren't lost. None is scheduled or designed in detail.

### Input fluency

A blank editor gives little guidance, and many completions are still missing.
This is the biggest weak spot.

- Completion offers come from provider functions that compose as combinators.
  Ranking and failure reporting, above, belong to the same design.
- A diagnostics layer attaches errors to layout as metadata, drawn like the
  selection highlight but red; clicking one shows its messages. It is a
  combinator over widgets and handlers.
- Types are one more combinator on top. A type declaration is, or generates, a
  guard function, and static checking proves the guards hold, so no red outline
  means no absents from failed guards. Start explicit, with no inference, and
  structural like TypeScript, using GID's identity tags rather than names.
- A result records one stack frame today. Capturing a full environment on
  demand, by re-evaluating with tracing, would explain a result without
  recording everything.
- Dragging a value onto a slot could link it, as Cmd-clicking does. The first
  newcomer test reached for dragging when it couldn't find picking.
- Enter and Cmd+Enter need rethinking together: while typing, which to press is
  hard to remember. Today Enter while typing ends the typing, Enter on a
  selection adds a peer beside it, and Cmd+Enter adds something within it, each
  offered by the view of the container it lands in. A selected value being typed
  in always keeps its caret, unlike a spreadsheet's cell, and that stays; Enter
  there only re-spells it and returns the caret to its default, where it could
  move on to the next thing, or in a list select an insertion after.
- A raw view of one value in place, as the document-wide Raw view shows
  everything. Structure a projection hides, such as a field on a number, would
  be authored there; today only a whole view can be raw.
- Wrap insertions: besides completions at a placeholder, libraries offer ways to
  wrap the selection. With `42` selected, asking for a wrap lists each operator
  with the 42 on its left or its right, and typing an operator such as `-` picks
  its left form. Typing an operator at the end of a number already does this
  for that one case.

### A second language

Grap is the language the examples are written in, not Progred's own, yet
newcomers take Progred for a Grap editor. A second language whose programs live
in the same documents, with its own projections and evaluator, would show the
difference, and would show where the editor still assumes Grap.

### Notebook and REPL

A Grap notebook in the style of Jupyter, but reactive like Pluto, marimo and
Observable: changing a cell recomputes the cells that use it, so there is no
hidden state from running cells out of order. Much of it exists already.
`{evaluate: expression}` shows an expression and its result together, results
project through the normal projections (pictures, tool previews, 3D views), and
observed evaluation tracks the cell definitions an evaluation reads. A notebook
is then mostly a projection of a list of named `evaluate` cells, with
ergonomics: a new cell on Enter, later cells referring to earlier ones by name,
and progress and cancellation for long computations.

- A REPL pane evaluates in the scope of the selection. After Cmd-clicking an
  output, the call that drew it supplies the bindings, as a debugger console
  does at a breakpoint. This builds on capturing an environment on demand.
- Entries are cells in a scratch list that can be moved into the document. Later
  entries refer to earlier results by identity, not by number.
- Entries that act, such as editing the document or exporting a mesh, need
  capabilities. The REPL is then the person-facing client of the same commands
  agents use, so it shares a design with Grap scripts.
- A prompt that answers whatever is typed is a familiar first step from a blank
  editor, which makes it part of input fluency too.

### External control and a newcomer test

The [session an agent drives](testing.md#a-session-an-agent-drives) exists, and
a first newcomer test went in blind: a subagent with no knowledge of the code
got a short description of Progred, how to drive the session, and a small change
to the Grap demo. It added an input but couldn't make the sum refer to it,
because Cmd-clicking a value into an open slot can't be found by looking; it
tried completion, dragging, and copy and paste. Progred needs some explaining,
so the next newcomer follows a guided tutorial as a reader on prog.red does,
which tests the tutorial too. Each report keeps friction in the tutorial, in
Progred, and in operating it as an agent apart. Add help only where a report
shows a need: the visible text and its boxes, captured as text leaves draw
(which accessibility needs too), `zoom`, or the computer-use actions left out so
far. Sessions could replay as scripts, so a fixed stuck point becomes a
regression test.

Later the live app could accept the same socket behind an opt-in flag, so a
person can watch, and an MCP bridge could serve Claude Code. An editor made for
an LLM is a separate goal: the same editor state and actions presented as text
and named actions instead of pictures and gestures, as in Phil Freeman's
comonadic UIs, where a component keeps its space of states and the actions
that move through it while its presentation changes. It needs actions as values
apart from the gestures that trigger them. Puri gets one opaque external event,
and a Progred combinator pairs a named action with its key binding and handler,
so projections opt in as they do for keys and can list what they accept. The
editor's structural fallbacks (paste, delete, insert, replace, navigation) move
into the handler chain as lowest-priority handlers. Completion offers already
work this way: an offer is data, and choosing one is a commit.

### Accessibility

The editor draws to a canvas with no accessibility tree, so screen readers
can't read it. A tree needs, for each element a person can reach, its role, a
name, its value, its bounds and the actions it accepts. AccessKit can hand such
a tree to each platform's accessibility API. Much of it exists in another form:
the frame's hit tests already identify every reachable element and its region,
but drawn text keeps only glyph IDs, so names and values would come from the
projections that draw them. External control needs the same description of
what's on screen, so the two can share it.

### Demo moments

- Drag handles on the tool profile picture that update the numbers it was drawn
  from, disabled where a parameter isn't a plain number or is read-only, plus
  Cmd-hover and Cmd-click from the picture to its source.
- CAM strategies a machinist recognizes, like Fusion's, built from constructs
  that can be inspected and changed, including patching a combinator's output.
- A chart of tool engagement through the cut.
- Small immediate delights, such as Cmd-hover to source and card literals,
  without turning the editor into a box of tricks.

### Decision tables

Jonathan Edwards's schematic tables in Subtext: a two-parameter match with a
grid projection that can check its cases are complete and don't overlap (compare
DMN hit policies and Parnas tables). Store the grid as nested records keyed by
the header cells' identities; each header cell holds a pattern, possibly with a
bind. This connects to guard-based types.

### Sharing and the Edwards write-up

Edwards asked to see the advantages. The one specific to structure is composing
semantics as in Lisp while the readable syntax is projected, an output rather
than an input. In the CAM example the clearest case is the tool definition that
previews the tool; much of the output display would work in a text system too.
Be as concrete about the costs: speed, no types, picker-based entry, `quote` and
`unquote` showing in lessons, outside tools seeing only the text format, and CAM
without G-code.

Turn the examples into lessons that run well in the browser and share the editor
link first. The owner rewrites the text in their own voice and sends the draft
to Edwards before posting. Sending the Fidget patches upstream is on hold.

### 3D printing

More accessible than CAM: many more people own printers, failures are cheap, and
the slicer owns the machine, so Progred only has to produce a good mesh.
`fidget-mesh` 0.5.1 already has `Mesh::write_stl`. Export needs a finer mesh
than previews (depth 8 gives cells about 0.4 mm wide on a 100 mm part), built in
the background, and a browser download.

- Definitions that preview themselves, like the tool profile: threads, snap
  fits, hinges, magnet pockets.
- Printability views, live while dragging a parameter: overhangs shaded from the
  field's gradient, the bed footprint, thin walls.
- Tolerances that know what they apply to. Printer calibration becomes a library
  applied to every hole and clearance; in OpenSCAD a hole is just a subtraction,
  so nothing can find them all.
- A deposition preview is the CAM stock with the sign flipped: it unions
  nozzle-width capsules along the path where the stock subtracts tool sweeps.
  That leads to designing extrusion paths directly, as FullControl does, and to
  slicing from composable pieces: perimeters as the field's contours, infill as
  a gyroid field. That is later work; slicers are mature.

Offsets and clearances need fields close to true distance, and min/max CSG
distorts distance near corners. Implicit modeling is weak at text and at
importing STLs for remixing. Prior art includes libfive Studio (by Fidget's
author), nTop, OpenSCAD and MakerWorld's parametric models. A good first demo is
a Gridfinity bin generator on prog.red: drag the compartment count, watch the
overhang shading, download the STL.

### PCB design

Prompted by the owner's keyboard project: an Apple Magic Keyboard's electronics,
Touch ID included, under a new mechanical switch matrix, designed in KiCad. The
pain points are many clicks per change, a schematic tied to one board, and no
way to abstract repeated structure.

In Progred the netlist is the data: parts, pins and nets as records. The
schematic is one projection among several: a net table, a row-by-column matrix
grid like the decision-table projection, and the board. Several designs can then
share or transform one netlist, for example a netlist reverse-engineered from
the original board and a new design that keeps its controller and connector nets
and replaces the matrix. A key layout generates the matrix and switch placement
(KLE JSON is the common layout format), so key spacing or diode orientation is a
parameter. The same layout could drive the switch plate and a printed case.

KiCad's files are S-expressions, which map easily onto GID. Read footprints and
symbols, draw them, and write a `.kicad_pcb`, or a netlist for KiCad's own
import, with placed footprints and nets. Leave routing, design-rule checks and
fabrication outputs to KiCad. Cell IDs are 128-bit, so they could serve as
KiCad's UUIDs and keep identity across regeneration, which must not clobber
traces routed in KiCad. KiCad 9's IPC API is another route, driving a running
KiCad.

The harder parts are readable schematic layout, which needs stored positions
except for regular structures like the matrix; routing, though generated routing
for a regular matrix is plausible; and KiCad's format changing between versions.
Prior art: Ergogen, SKiDL, atopile, tscircuit.

### Editing generated output

Generators that write files for another tool only work one way. Hand edits to
the output, such as mounting holes or a nudged part, are lost or redone each
time the generator runs again. Keep those edits in the document instead.

- Where a generated thing comes from a literal or a simple expression, dragging
  it edits the source, as with the tool-profile handles.
- Otherwise an edit is an override, applied to the generator's output after it
  runs and stored with the generator in the document. Patching a CAM
  strategy's output is the same mechanism.
- Shared definitions replace aligning one tool's output in another: the board's
  mounting holes and the case's standoffs come from one list.

An override targets an output by a stable identity: its path through the
computation, made of call sites and, at each loop or map, the identity of the
element being processed rather than its index (the switch for key Q, not item
17). Code refactors change call sites, so a generator can also tag an output
with an explicit identity, as React keys do. CAD's topological naming problem is
what happens without this. Drawings already carry a source trace and a call
trace, which Cmd-click to source uses; values that evaluation mints with fresh
random IDs would need IDs derived from their path instead. See also address
lifetime, above.

An override records the generated value it was made against, so regeneration is
a three-way merge: an override is a patch and regeneration a rebase, sharing
machinery with version control. It applies cleanly when its base is unchanged.
When the generated value changed underneath it, or its target is gone or now
ambiguous, it fails explicitly: the diagnostics layer marks it, and the UI
offers to keep it against the new value, pick a new target in the output, or
drop it. Nothing is silently lost.

Work done in an external tool, such as routing in KiCad, comes back the same
way, imported as data keyed by identity, so a regenerated board keeps its routes
where they still connect.

### Version control

Separate from the domains above, but needed for Progred to be complete. GID's
identity-keyed cells suit structural diff and merge better than text: a change
is a field of a cell, and two branches conflict only when they change the same
field of the same cell. A diff is data, so the editor can project it: changed
values highlighted in place, and geometry before and after overlaid, much as the
CAM view shows old stock desaturated beside new while it updates. Visual diffs
would be a real draw for hardware, where comparing versions of a board or a
model is painful.

A first step can sit on Git. Saving writes canonical text, so a diff driver and
a merge driver for `.gid` files, keyed by cell ID, would work in ordinary
repositories. Undo history is the starting point for history inside the
document, with named versions, branches and merges built on it. Large blobs such
as scans need content-addressed storage rather than inline text.

Provenance, such as when, where and by whom a value was made, could live in
fields on the value itself. Views that draw a value as one piece show its other
fields after it, so they'd need a way to tell such metadata from the value's own
fields and leave it out.

### Multiplayer editing

Several people editing one document at once, perhaps with CRDTs. Cells are
already keyed by identity, and list elements already have fractional positions,
the kind of identifiers sequence CRDTs use. Those positions are session-only
today, minted at load and dropped at save; see saving list positions above.

### A common thread

CAM, printing and PCBs share a shape. Progred owns the design logic and its live
projections, and hands off to a mature tool for the last step: a CAM
post-processor, a slicer, or KiCad.

The keyboard could tie these together: a whole product, board, plate and case,
in one document with shared definitions and version control.
