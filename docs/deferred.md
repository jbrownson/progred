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
Grap lambdas, cases, binding clauses, and control/operator calls decline when
they cannot show an active insertion. That fixes those specific projections;
it is not a general fallback policy for arbitrary facets such as line editors.

## Discoverability of expression definitions

Keep Grap expression references shallow, including the source shown by
`{evaluate: expression}`. The toolpath example places an ordinary reference to
the ball-tool cell immediately before its evaluation so the normal projection
exposes its editable contents. Both occurrences reference the same definition;
neither is a special declaration. Revisit navigation or expansion affordances
if finding these definitions becomes a recurring problem, rather than changing
`evaluate`'s projection now.

## Selection destinations and history

Plain selections work with line editors' missing-state defaults, so paste,
deletion, and undo do not need editor-initialization hooks. Destination policies
may still merit customization: deletion chooses a surviving sibling or parent;
undo restores an edge location, not its prior caret or pending query. Preserving
those states is a separate UX decision, not necessary to make restored selections
usable. Never resurrect active drags or IME composition from history.

Line-editor conversion runs after every accepted event, including caret
movement; equal results already skip document writes. Avoiding unnecessary
conversion is a separate optimization. Projections must not rely on that
optimization to keep a missing value absent: a direct line editor offers its
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

## Libraries with native code

A library that needs native code, like the fidget library around the Fidget
crate, could someday load as a plugin built from one source: a native dynamic
library on macOS and Linux, and a WebAssembly module on the web. It would bring
what an editor-authored library does, plus native FFIs that take GID values and
return one, with images and meshes as blobs. Long calls would run as jobs with
cancellation, progress and partial results, and the editor would draw their
results with widgets it owns: an image, a mesh, an orbit viewport. Values would
cross as bytes, so this needs a binary GID encoding first. A native plugin isn't
sandboxed, so it's installed deliberately, never loaded because a document
names it; on macOS it must be signed by the same team unless the app turns off
library validation. On the web, a separately loaded module can't join the app's
worker pool, so it runs single-threaded unless it brings its own workers or the
host splits its jobs.

None of this is planned, but the fidget library should stay separable:

- Calls into the Fidget crate stay in the fidget library and the `volume` and
  `cam` crates. Fidget objects, such as a lowered `Tree`, never go into Grap
  values; memos keyed by the field value cache them.
- The GPU handoff stays at pixels and meshes; the fidget library renders on its
  own device.
- Other libraries use the fidget library's vocabulary, not its functions. The
  toolpath library is today's exception: its previews reuse the fidget
  library's scene, raster, mesh and viewport code, so the two would leave
  together unless that 3D preview code becomes the editor's.
- Library handlers reach the editor only through the session, and background
  jobs own their inputs. Since 18 October 2026 the libraries are generic over
  the editor, which holds them to it; a plugin's FFIs would cross as data, so
  no generic crosses a plugin's boundary.
- FFIs evaluate their named arguments and return data. Toolpath's path
  commands, which apply Grap functions to each point and emit effects, would
  need the host to record commands as data, or calls back into the evaluator.

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
- Keyed controls (`radio`, `tree range`, `tree cursor`, and
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

## Lexical parameter lookup

A call binds a missing argument to the missing-argument absent, so a body
that doesn't use that parameter still answers, and one that does reads the
absent there. That covers parameters a call leaves out, but not a parameter
reference with no binding at all, such as one in a body evaluated outside its
call: a reference the environment doesn't bind falls back to the cell's
definition, which for a parameter is its name record. Resolving parameters
lexically, by the lambda that declares them, would make such a reference an
absent of its own rather than the parameter's name.

## Where an absent came from

An absent says why a value is missing, and its chain says what caused that
(see [projections](projections.md)), but not where: which call made it, under
which bindings. For debugging, each absent could carry the stack and
environment it was made in, or in a debug mode a whole evaluation trace, so
an absent shown in the editor leads back to the code that produced it. The
display for this isn't designed yet.

## Context for an absent from below

A function written in Rust reports the arguments it couldn't use in its own
terms (see [projections](projections.md)): `area {width: "three"}` reports
that the `*` inside `area` couldn't use its `left`, not that `area`'s `width`
was wrong. That's honest, since the function that failed knows only what it
needed, but it won't always have the useful reason. A higher-level construct
could add context when something below it fails: wrapping an absent that
comes out of it with its own reason and what it was given, as exceptions
chain context. Undecided: whether that happens at every function boundary,
which costs on the evaluator's hot path and deepens chains, or only where code
asks for it, and how it meets the chain limit. Related to *Where an absent
came from*.

## Address lifetime

Paths and source traces already preserve definition source. A future identity
review should specify what survives list moves, deletion/undo, definition
replacement, and transient-root changes. A cell has meaning in a resolution
context; neither a path nor a cell-relative trace is a global address.
Keep occurrence identity, cell identity, definition source, and view identity
distinct while answering those questions.

## List positions

List positions are saved with a document (October 2026), so a path through a
list survives a reload. Left for later:

- **Normalizing.** Positions only grow: appends and prepends a byte per 255,
  inserts into one gap about a bit each. A manual command could spread a list's
  positions evenly again, invalidating saved paths into that list, and the
  editor could suggest it for an unusually long one.
- **Concurrent edits.** Minting is deterministic, so two people inserting into
  the same gap at once would mint the same position; a CRDT would append a
  per-replica tiebreaker, which positions' open-ended bytes allow without a
  format change. Concurrent runs at one spot can interleave, which newer list
  CRDTs such as Fugue avoid.
- The clipboard's JSON form still keeps only the order.

## Native GID storage

GID's native binary representation isn't designed yet; see [GID](gid.md). The
`.gid` extension is reserved for it: when it lands, it takes over the extension
and the [text bridge](gid-text.md) moves aside.

## Names beyond one string

The owner's direction, from 18 October 2026: a name will become more than a
simple string. A thing may have several names, for different languages and the
like, maybe even iconic ones, so nothing should build in how a name projects,
only which field holds it. Searching by name then asks how closely a name
matches a string, rather than getting a single string from the name.

A first step is in: a name is text or a formatted name, a list of parts, each
text, a cell spelled as that cell's name, or `{subscript: part}`, and
`name::of` reads either as a `display::Name`. Field labels, references and the
picker's rows all draw it, and the number operators use it for their
representation subscript. Its spelling, the parts' text joined, still sorts
names and is what the picker's query filters and ranks; a match query in
place of that filter, and names projected rather than read, remain. The `.gid`
printer still binds only plain-text names, and only plain names have an editor.

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
  Grap's slots do.
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

- The picker as a widget a projection uses, not special machinery. This is the
  owner's direction from 17 October 2026.
  - A projection puts a picker in a hole and supplies its offers.
  - Raw becomes one more projection supplying its constructors and typed
    values, instead of being the picker's fallback mode.
  - Choosing an offer acts through the interface the editor gives projections
    (see [Code size and structure](#code-size-and-structure)). That interface
    also supplies what a picker needs to read, such as names and libraries.

  Today, by contrast:
  - ~~Raw's offers are built inside the editor's picker code, and Grap's offers
    borrow them from there.~~ Since 17 October 2026 they are display's, beside
    the ranking of a projection's offers, and Grap's slots compose them.
  - ~~The engine builds the picker around display's card.~~ Since 17 October
    2026 the picker is a toolkit widget a projection draws with its own offers,
    and Raw's picker is one more picker, its brackets the picker's keys.
  - ~~Choosing an offer receives the whole editor.~~ Since 17 October 2026 it
    acts through the session, committing into the open picker's `Picking`.
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

Where offers come from, as of 20 October 2026, with the owner's view of each:

- **Starting points** (`CompletionScope::Start`): what a document starts as,
  Grap's new program and Fidget's new scene, for an empty root's hole. These
  stay a category of their own: they are how a document gets into a custom
  projection, whose holes then offer its own. They may not need to be
  root-only; that's undecided.
- **Typed readings** (`Literal`): what's typed, read as one of a library's
  values, such as `12` as a number, `"hi"` as text, or a new field's label named
  as typed. Raw's holes ask every shown library for them. The owner doesn't
  want this as a category of its own: any offer could instead be contingent on
  what's typed.
- **Offers for anywhere** (`Everything`): color's new color, and Grap's new
  lambda and calls by name. Only Grap's slots ask for them, and new color is
  the only answer from outside Grap, so dropping the category would leave
  color no way into a Grap slot. The owner considers the category the wrong
  answer to a real problem, and would rather leave it as it is than patch it,
  to be rethought here.
- **A projection's own**: Raw's new cell, list and record; Grap's bindings in
  scope and quote and unquote. Grap knowing control's binding forms and quote is
  fine: they are Grap's macros, which don't follow the parameter lists offers
  are otherwise computed from. Declaring such forms instead of coding them can
  come later.

Paste is the same question from another side. It reads text outside the
providers (`selection::resolve_query`, and `clipboard.rs` spelling text), so
pasting `12` makes text where typing it offers numbers, and paste makes text
with the text library off. Typed readings, paste and the text file format all
spell values as text; the owner would like them to share code with the
serialization text bridge, with special encodings for core types such as
strings and numbers.

## Starting points

A document's empty root offers the libraries' starting points, then Raw's
offers. There should be one for every direction a person might want to go, each
landing on something that works. Today's two, Grap's new program and Fidget's
new scene, each lead somewhere awkward; Grap's opens with a missing-cell error
in its pane until the program is written (that error's hole does take the
program now). A Grap notebook in the style of a REPL should be a starting point
of its own beside Grap's; see [Notebook and REPL](#notebook-and-repl).

## Workspace documents

A document whose root has a `panes` field is a workspace, and the editor
swallows it: it shows the panes, and a view of the workspace record starts at
its `main` value ([the model](model.md#panes)). Plain
and Raw, or turning the editor library off, show the whole record. A document
is wrapped only once something adds a workspace field, such as opening a pane.
Starting points that open a pane and the examples use this shape. What's left,
discussed in October 2026:

- **Recursion.** A pane showing a workspace record already draws its main value;
  drawing that workspace's own panes too suggests making the pane layout a
  projection.
- **Tabs or splits.** A main value holding several things, such as a Grap
  program and a Fidget scene, could draw as tabs or splits. That's a projection
  of a record of several things, wherever one appears, not part of the
  workspace convention.

## Islands in the document

Deleting a value unlinks it, but cells it referred to stay in the cell table
and are saved, so documents collect cells nothing can reach. Nothing collects
or shows them; only a test keeps the checked-in examples clean. Thoughts from
October 2026, to try iteratively:

- **Collect unreachable cells,** first at save, so a saved file never holds one.
  Undo already restores a deletion, since its snapshots hold the cell table.
- **Copying carries the cells a value refers to,** so a cut value still pastes
  whole later, and pasting into another document works. That makes collecting
  at delete time safe too.
- A panel listing islands would mostly be a recovery tool, which undo covers.
- A cell that's still reachable but drawn nowhere is a different case, as when a
  notebook entry other entries use is deleted from the notebook. Following a
  reference still reaches it; see in use whether its uses should look
  different.

## Line breaking

Each container offers a one-line form and a stacked one, and the first whose
natural width fits wins, so on a wide window nearly everything goes on one line,
a small root document included. Raised in October 2026 and deferred until there
are more examples. Reasons something might stack even when it fits:

- Several things rather than parts of one: a document's top level, definitions,
  `do` steps, `match` cases and bindings read down; a call's arguments or a
  point's `x` and `y` read across.
- More than one compound child, many labelled fields, or nearness to the root.
- Reading measure: a one-line form should fit about 70–80 characters' worth, not
  the window. Wadler and Leijen's pretty printers call this the ribbon.
- Stability: rules from structure don't flip while typing; width rules do.
- Manual control as an option, stored like folds. Making Enter imply stacking
  would stack everything built by hand, since Enter is the only key that adds
  fields; an explicit toggle is safer.

A research pass found the website already works around it: the tutorial embed
forces a lesson's root into a column (`web_embed.rs`), the growth lesson's "click
between the last two tree lines" holds only because its embed is narrow, and
`navigation.gid` asks the reader to narrow the window. On a laptop-width window
the IoP tree's scene body, the forest's drawing steps and a `where` in
`libraries.gid` sit on one line. The rules it proposed need one engine
primitive, a hard break, which keeps any enclosing layout from taking its
one-line form (Prettier's `breakParent`). More stacking also means more labels
over indented values; avoiding that staircase needs a form that keeps a
multi-line last item beside its head, which the engine can't express yet.

## Code size and structure

An audit on 16 October 2026 ([the evidence](history/codebase-audit-2026-10-16.md))
found almost no dead code: 60% of progred's 94k lines are tests, much of them
inside source files, and 93% of its production code is one dependency cycle.
Its first recommendation is done: inline test modules now live in their own
files. projection/tests is still under projection/. It held 20.8k lines that
mostly test the frame and the whole editor, and is down to 6.3k as suites move
to their own modules (item 4). The private items it reaches point at misplaced
code, not at internals the tests should see. A follow-up
[crate split analysis](history/crate-split-2026-10-16.md) maps the coupling
that blocks a split and the steps that break it. The rest, most worthwhile
first:

1. ~~Keep docs/history out of default searches, and point agents to the doc
   sections they need.~~ Done on 17 October 2026: ripgrep skips docs/history
   unless the directory is named, and AGENTS.md says which docs are large
   references to search rather than read whole.
2. ~~Share one `display::Env` test double and the common evaluate, apply and
   projection-input test helpers, now copied 19 to 29 times.~~ Done on 17
   October 2026: display's test support has the inert and one-answer hosts,
   and the libraries' test widgets build a projection's input and targets.
   Eight hosts stay with their tests, since they check their arguments, count
   calls or answer one test's definitions; the evaluate and apply helpers were
   already shared, and the copies left bind each module's own functions.
3. Give projections and libraries a clear interface to the editor: a stated
   subset of it rather than the whole editor. This also makes libraries
   plug-ins that can't reach menus or the window. The owner's direction, from
   17 October 2026:
   - **Read side.** It partly exists already. The host services a projection
     calls (evaluate, apply, a cell's name and definition) are an interface
     that the document's reading context implements. What a view can see while
     it's built is an ad hoc bag defined inside the projection engine, and it
     would become the rest of the read side. The selection belongs in it.
     Folds are one convention of the per-path state it carries.
   - **The theme.** It feels more like a combinator than part of the
     interface: a projection says what kind of text something is, and the
     toolkit applies the theme, as the picker already does. Projections that
     render pixels still need the zoom scale.
   - **Write side.** It replaces handing handlers the whole editor. It covers
     edits where they are allowed, the selection, per-path state, drags, and
     the picker.
   - **Text in Grap.** Done on 17 October 2026, at the owner's direction: Grap
     owns text's encoding beside f64's and unboxes text as it unboxes numbers.
     More number formats will come later, with no rush. A new one adds a native
     form to the runtime's unboxed values, moves its encoding into Grap as f32's
     and u64's would, and reads and builds through `RuntimeValue` like f64.
   - **Library vocabularies.** The core libraries' vocabularies, such as
     text, names and numbers, can stay referenced by the editor, and the
     library interface can grow where that removes a reference. The
     [audit](history/library-vocabulary-audit-2026-10-17.md) found 257
     references:
     - 79 are the registry.
     - 54 are core vocabulary to keep. 28 of these called Grap's conventions
       through library re-exports; the core now calls Grap directly.
     - 64 are vocabulary only the core interprets: selection stages, per-site
       state, folds and panes. On 17 October 2026 these moved into the core
       beside the code that interprets them, with the libraries re-exporting
       the cells, and the library interface itself (`Library`, `Libraries`,
       `Definitions`) moved to `crate::library`.
     - 60 are the core doing a library's job. Most of these were the engine
       interpreting the layout library's drawing programs; since 17 October
       2026 the layout library records them itself into the toolkit's
       recorded-drawing leaf. The rest are:
       - presentation's pane special cases, as a pane entry and a wrapping
         projection;
       - ~~Tab's skipping of name slots.~~ Done on 17 October 2026, at the
         owner's direction: Tab visits tab stops, which a widget marks on its
         occurrence as it registers an arrow stop. The picker marks every
         empty, writable hole, so an unnamed value's name is one, while an
         unnamed lambda's `λ`, a marker to click, isn't;
       - ~~the cells that label Grap handlers' event records~~. Done on 18
         October 2026: the records stay, since Grap handlers need values, and
         the core's `site::event` owns their cells and encoding, which the
         layout library re-exports.

       Plain was one of these. The owner decided on 17 October 2026 that Plain
       is a list of libraries, not something each library provides. One list
       in stack.rs now serves the editor and the tutorial page, which had a
       narrower copy.

   [Design notes](history/projection-interface-design-2026-10-17.md) work out
   its three larger pieces with signatures and landable steps: handlers acting
   through a session of capabilities instead of the editor, the picker as a
   widget with Raw's offers as an ordinary projection's, and drawing programs
   recorded by the layout library into a toolkit widget. Done on 17 October
   2026: the session note's first five steps (the view aliases, the command key
   read from the frame's input, the session that edits act through, and
   gestures and offers acting through it too) and its sixth: actions and
   targets act through the session too, which a first `Host` lends; the
   session holds the drag, so starting one, folding and picking need no
   editor; and widgets act on their place through an opaque `Site` instead of
   carrying its view, path and scope; since 18 October 2026 a projection's
   input lends its occurrence's site too, rather than closures. The seventh is
   done too: computations are always supplied; a library's widget reads its
   view through `Viewing` (the scale, writability, a Grap host, the
   computations, and the state and selection below it), keeps memoized work
   for its `Site`, and names a `Face` rather than reading the theme; and the
   toolkit reads through `Toolkit`, so nothing below the runtime names its
   per-view bag. The outline's sections jump to a site rather than a
   `Location`. Also done: the picker note's first four (the card, its
   placement and the selection ring moved to display, then Raw's offers and
   the ranking of a projection's, then the picker itself as a toolkit widget,
   then `Picking`, the open picker the session lends, which offers commit into
   and the query and card edit, so none of them takes the editor), the drawing
   note's first two, and half its last: the source-link handlers are the
   toolkit's. The picker note's fifth, typed picker state, is dropped: the
   owner decided on 17 October 2026 that the picker's choice and scroll stay
   in the selection's payload, since nothing should hard-code the picker,
   which a Grap library should be able to recreate. A site's edit says
   whether it's selected and what's typed there, as the session note's
   `Edit::selected` does, so number's operator keys read the selection
   through it. The eighth is done since 18 October 2026: `Host` names what its
   world's pointer rests on, and display and the libraries are generic over
   it, the recorded leaf included, so none of them names the editor or the
   shell's `Hovered`. What handlers reached past the session for now goes
   through it: landmark arrival, the picker's Tab and the source links reveal
   through the model, Enter's authoring is a session function, and the
   source links read the pointer and modifiers from the dispatch context.

   The crate split analysis's first four steps start on this, and each pays
   off on its own:
   - ~~alias the concrete `<Editor, Hovered>` pair, written out 332 times;~~
     done on 17 October 2026, as aliases the projection runtime keeps for
     the editor, and frame.rs for its hover;
   - ~~let the model own the vocabularies and codecs it interprets, as grap
     owns f64's convention;~~ done on 17 October 2026;
   - ~~back the session edits act through (`editing::Session`) with the model
     and the libraries rather than `&mut Editor`, and let the shell set the
     window title after dispatch;~~
     done on 17 October 2026;
   - ~~move the hover types and the completion helpers down, and route calls
     into the projection runtime through `Project`.~~ Done by 18 October
     2026: the hover types and the picker's commit refer only to the core,
     and display reaches the runtime only through `Project`. The per-view bag
     stays the runtime's, which display reads through `Toolkit`.
4. Move projection/tests' frame- and editor-level suites to where they belong,
   leaving only the runtime's own tests; nothing stays anywhere for historical
   reasons, and nothing became visible just for tests. The
   [survey](history/projection-tests-survey-2026-10-17.md) of its 324 tests
   orders the work:
   1. ~~Move the 46 already-targeted tests to their own modules.~~ Done on 17
      October 2026, with the tutorial-slot and event-encoding tests once the
      harness had moved.
   2. ~~Move the 39 library projection tests next to their libraries, through
      the shared harness and the public `project()`.~~ Done on 17 October
      2026. The harness is now `projection::test_support`.
   3. ~~Move the completion card tests to display, with the card.~~ Done on 17
      October 2026. The one that places the card over a projected editor stays
      with the picker flows, for step 5.
   4. ~~Give the profiling tests both projection phases.~~ Done on 17 October
      2026. The harness is the projection runtime's own test support, so it
      times both phases without exposing them. The layout FFI tests moved to
      the layout library and the Fidget render check to the Fidget library.
   5. ~~Give the driver the verbs that end-to-end tests now reach around it
      for, and move the 67 of them onto it.~~ Done on 17 October 2026. The
      driver selects a path through its landmark, reads the selection's stage
      and the edited line's caret, reads and chooses the picker's entries, and
      takes input-method text and the window's focus, appearance and
      libraries. Each flow went to the code it exercises, not to a suite of
      its own. Resizing and stepping background work wait for a test that
      needs them. The runner's own tests, the Fidget
      interaction's timer tests and one navigation test still drive the
      runner: that one checks the next key's handlers are installed without
      a paint, which the driver always does.
   6. Rewrite 30 tests that drive the whole editor but check one system. The
      event encoding has moved to the layout library. What's left is mostly
      two groups. Nine write-through tests in projection/tests/editing.rs
      get their editing selection by projecting a frame and pressing End;
      targeted, each would build it from its library's own line editor,
      mounted with `Selection::edit_line_mut`, before `editing::commit_line`.
      Seven in frame/layout.rs check the toolkit's placement, hover and
      dispatch through a throwaway projection in a whole frame; targeted,
      they'd measure and place the layout in display's tests, which first
      needs display's test support to hover and dispatch a placed layout.
   7. ~~Give the captures and the frame canaries modules of their own, with
      the SVG writer in test support.~~ Done on 17 October 2026. The SVG
      writer and its captures are `crate::svg`, and the frame canaries
      `crate::perf`'s, adding their partials before the view adds its entry
      rather than replacing the projection's private one. The full-editor GPU
      tests went to the shell's tests and the two GPU mesh tests to
      puri-vello, with the image round trip they compare against in its
      `capture` module. The lesson tests went to the tutorials, the failed
      toolpath preview to the toolpath library, and the example editors they
      share to `crate::test_examples`.

   CAM tests took 84% of progred's test time, running Fidget's generic
   meshing and rendering and the cutter geometry at progred's debug level. On
   17 October 2026 the meshing moved to the optimized `volume` crate, which
   cut the slowest test from 10 s to under 1 s and progred's tests from 22 s
   to 13 s. Item 7 moves the rest.
   - ~~The completion card, its placement and the selection-ring drawing
     (`completion_card`, `completion_placement`, `primary_highlight`) are
     display code filed in the projection runtime.~~ Moved to display on 17
     October 2026.
   - ~~`projection_target` builds display's `ProjectionTarget` from `Cx`, and
     moves down with it.~~ It stays in the runtime with the bag, and its test
     with the runtime's.
   - ~~The frame harness calls `prepare_project` to time the projection's two
     phases.~~ It still does, as the runtime's own test support.
   - ~~`Projection::without_entry` serves one profiling test.~~ The canary
     builds its bench without the entry instead.
5. ~~Give the six things called "source" distinct names.~~ Done on 17 October
   2026. Where a cell's definition comes from is a `resolution` everywhere, as
   gid's `Resolution` names it, and the `DefinitionSource` alias is gone; a
   scope answers an occurrence's `location`. "Source" now means the document
   and libraries values are read from (`Sources`) and the code that drew
   something (`SourceTrace`, `source_link`, `SourceCalls`).
6. ~~Give the occurrence-to-location mapping one owning module, and pass an
   opaque handle through display and the libraries instead of `Location` and a
   conject.~~ Done on 17 October 2026: a scope maps occurrences to where
   they're stored, a library names a place by its `Site`, and only the
   toolkit's jumps and source traces see `Location`.
7. Move the rest of the volume and CAM geometry out of progred, into crates
   built optimized like the other dependencies (see item 4). Meshing moved to
   `volume` on 17 October 2026, and the same day the cutter profiles and
   sweeps, stock and toolpath recordings moved to `cam`, with the tests that
   need no document: their dense reference checks went from about 11 s, run
   one after another, to under half a second, and progred's tests from 14 s to
   11 s. The toolpath library keeps a cutter's document encoding, through
   `ToolValue`, and its picture. Still in progred:
   - the software raster and its tile assembly (libraries/fidget/raster),
     which `volume` could take once a render request takes a camera rather
     than reading one from the view's state. Moving it would also build the
     app's software previews optimized in development, since Fidget's
     generic scene rendering is instantiated where it's called. The CAM
     profile reaches four of its internals (`refine_depth`, `view_at`, the
     request's pixels and `software_tiles`), so those would move behind a
     profiling feature of `volume`'s rather than become its API;
   - the playback core and the tube meshing (libraries/toolpath/playback.rs
     and mesh/tubes.rs), whose generic paths still compile at progred's
     level.
8. Split progred into three crates, which would also keep the layering honest:
   - core: the model and display;
   - the libraries;
   - top: the projection runtime, the shell and the app. The shell and the app
     stay together, since the editor is specific to its platforms.

   The `Host` trait that makes display and the libraries generic over the
   editor came on 18 October 2026, and since that day nothing in core or the
   libraries refers to what would sit above it, outside tests. Raw draws and
   offers by GID's kinds alone: it asks the libraries what typed text reads
   as (text, numbers, a new label) and reads blobs in GID's own hex spelling.
   The name library reads and makes names, and the toolkit asks its
   `Toolkit` for a cell's name rather than decoding one. Navigation's
   geometry is display's, leaving only the keys in the shell's `navigate`.
   The CAM profile loads its document at the top.

   The tests are what's left, and they are why the split waits. On 18 October
   2026, 27k of the 33k lines of core and library tests named something that
   would sit above them: the editor as their host, its library stack, the
   projection runtime, frames or the driver. A crate's unit tests can't reach
   a crate that depends on it, so the split would either move those tests
   into the top crate, away from the code they check, against item 4's aim,
   or wait until they're targeted, testing through display's own hosts as
   item 4's step 6 starts to. Do it then, after a `cargo build --timings`
   baseline: items crossing a crate boundary would need `pub` rather than
   `pub(crate)`, and the libraries, generic over their host, would be
   instantiated in the top crate. The saving is in lines, not measured time,
   and `make test` could get slower for edits to core.

## Reporting completion failures

Offer construction decides applicability; activating an offered completion
consumes the input. Its Grap continuation can still fail at runtime. For now,
unsuccessful preparation installs neither the insertion nor its staged
selection/annotation changes. Declining after an effect is an evaluator
contract error, and like other completion failures it is silent.

Later, design an editor-owned way to surface these failures, such as alerts or
notifications, using the failure details in GID values. Do not add that machinery
now. Explicit decline is distinct from ordinary absent results, which are
definitive values and retain their effects. The eventual reporting policy must
not assume every absent warrants an alert.

## General computation reuse

The [computation graph](incremental.md) serves CAM geometry; canvas programs
still record once per visible frame, and other projections aren't converted.
Grap path generation is synchronous, and there are no Grap-language memo or
async boundaries, user-facing quality controls, or durability tiers. A
Grap-language memo would still live above the evaluator: Grap would hand its
host the evaluation to reuse and its key, and the host would keep the memo, as
the editor's `computations::memo` does for Rust boundaries. Future readers of
ordered contributor sets must observe that set, not just a selected
definition. Do not add an ad-hoc cache or event-specific relevance checks
alongside this mechanism. Incremental lambda calculus and incremental Fidget
algorithms are separate research.

The [2026-09-04 tree profile](history/tree-profile-2026-09-04.md) records measurements
from that implementation; timings are historical observations, not guarantees.

### Runtime results across evaluator boundaries

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
Do not preserve a creator's dynamic stack as a caller. Keeping runtime values is
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

## Grap value size and argument lookups

Measured in October 2026: `RuntimeValue` is 64 bytes, a 56-byte kind whose
largest variant is an inline closure (48), plus an 8-byte cache of its GID
form, and `Halt` carries a 24-byte `Value`. A result over 16 bytes returns
through memory rather than registers, which covers most of the evaluator's.

- **NaN-boxing**, making every value 8 bytes, was deferred in August 2026,
  since staging and the unboxed-number paths took most of the win, and it
  rewrites the representation throughout the evaluator. The owner's rule: worth
  doing now only for a large multiple, or for a smaller gain once the model
  stabilizes. Decoding must canonicalize the NaN bit patterns documents may
  carry.
- Putting the closure variant behind a pointer, taking the value to about 40
  bytes, read slightly slower when tried.
- Putting a halt's value behind a pointer would bring small results (`()`, an
  f64, a bool) to 16 bytes or less, in registers. The sizes are measured, the
  gain isn't.
- Finding an argument by label (`Context::field`, a binary search over 16-byte
  cell IDs plus a handle clone) cost about 84 instructions a lookup, 4% of the
  IoP tree canary, until `ForeignFunction::with_arguments` found the ported
  functions' arguments once per call site. Its other callers, such as `quote`,
  drawing records, `do` and list functions that read their arguments by hand,
  could stage too.
- The generic parsing costs more per call than the hand-written reads it
  replaced; staging more than made up for it, leaving the IoP tree canary 1.07%
  below its count before the construct, from 1.92% above.

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
to the record view. It would be the second memo of the kind AGENTS.md allows
beside `incremental`: caller-owned, keyed by its whole input and swept each
frame.

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

Ordinary arrows use [layout-derived navigation](navigation.md). Placeholder
Tab navigation, grow-selection commands, directional history, and moving a list
item with a modifier-plus-arrow shortcut remain separate decisions. The range
controls also remain pointer/touch-only.

## One control-state model

The plain `slider` hands each change to its Grap `on change` handler, which
replaces the control state through `update`; the radio, the tree controls and a
tree cursor's slider still store their values under keys in that state
themselves. Moving them to the slider's value-and-handler interface would leave
one model.

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

### The platform's drag threshold

A press starts dragging once it moves 3 logical pixels
(`gesture::DRAG_THRESHOLD`). Each platform has its own threshold; take it from
the input adapter once that exposes one.

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
  What either adds is also decided in one place, `pending_enter` and
  `pending_insert` in selection.rs, by the kind of value: beside an element a
  sibling, on a record or a cell holding one a field, on a bare cell a value.
  A container's view only accepts or refuses it, so a Grap call can't make
  Enter add its next argument, nor a match its next case. It belongs with the
  projections that draw those containers, Raw's list, record and cell views by
  default, asked by the container when the selection is inside it.
- A raw view of one value in place, as the document-wide Raw view shows
  everything. Structure a projection hides, such as a field on a number, would
  be authored there; today only a whole view can be raw.
- Wrap insertions: besides completions at a placeholder, libraries offer ways to
  wrap the selection. With `42` selected, asking for a wrap lists each operator
  with the 42 on its left or its right, and typing an operator such as `-` picks
  its left form. Typing an operator at the end of a number already does this
  for that one case.

### Languages as peers

Grap is the language the examples are written in, not Progred's own, yet
newcomers take Progred for a Grap editor, and it is one more thing an
introduction has to explain. The direction is JavaScript, Python or a
WebAssembly engine as peers of Grap, any of them able to write everything Grap
writes today: presentations, layout programs, controls, event handlers and
computations. Grap joins through the same interface as the others, and could
still grow into a typed language; making it fast is its own project. A
language could later arrive as a plugin, as the libraries with native code
above might.

A language would be a library that brings an evaluator:

- It claims code values by its vocabulary (`{js: "…"}`, or Grap's lambda and
  call records) and prepares them into callables. The editor tries the loaded
  languages in library order and the first that accepts wins, as partial
  projections compose.
- Callables take and return a neutral value: GID data or another callable.
  Native functions do the same, taking evaluated arguments, with a lazy
  parameter arriving as a callable, as the note on native code above already
  asks.
- Everything a language reaches goes through the host: resolving cells, so a
  JavaScript function can call a Grap one stored in a cell; dependency
  tracking, which keeps per-occurrence memoization working; effects; a budget
  in place of Grap's fuel; and the call's own capabilities, such as an event
  handler's get, set and path.

The editing core, layout, `incremental` and grap's `Host` are already
language-neutral. The coupling is in four places:

1. Display's value type is `grap::RuntimeValue`. It would become the neutral
   value, with Grap's closures and unboxed numbers staying inside Grap.
2. Every library's native functions take Grap's `(Context, Expression,
   Environment)` and read their arguments with the same few lines, repeated
   about 66 times. The new signature replaces both.
3. Display's `Env` evaluates and applies Grap. It would call a callable,
   whatever its language.
4. Click-to-source reads Grap's per-call origins. A neutral origin is a cell
   and a path; a language that can't trace its calls reports its code cell.

JavaScript is the cheapest route to demo quality: the web build already has
the browser's engine, macOS has JavaScriptCore, and QuickJS covers Linux, or
every platform if one engine everywhere matters more. A WebAssembly engine
gives many languages, fuel metering and sandboxing at once, and is the better
plugin story, since a sandboxed module can be loaded more freely than native
code. Its values cross as bytes, which needs the binary GID encoding above, so
code calling native functions in a tight loop pays for each call. Python is
the heaviest to embed.

On 20 October 2026 a recursive fib(30) took 8 ms in V8 with its JIT, 76 ms in
V8 without it, 86 ms in native QuickJS, 148 ms in QuickJS compiled to wasm, 548
ms in Boa and about 1.6 s in Grap, whose calls carry fuel, call records for
click-to-source and environments keyed by cell. QuickJS ran about 560
instructions a call to Grap's 4,600. That favors QuickJS as the one engine for
every platform: small, interruptible, sharing the host's memory, and fast
enough.

Open questions: whether code is stored as source text, which engines take, or
as structure; and how JavaScript sees records keyed by cells, since
`shape.radius` needs names, which the libraries' name records could supply.

In October 2026, Grap's evaluator and its values took 70% of the IoP tree
benchmark's instructions, malloc not counted, 24% of the layout FFI
benchmark's, 19% of the forest slider's and 13% of CAM scrolling's, where
Fidget took 25%.

### Notebook and REPL

A Grap notebook in the style of Jupyter, but reactive like Pluto, marimo and
Observable: changing a cell recomputes the cells that use it, so there is no
hidden state from running cells out of order. Much of it exists already:
results project through the normal projections (pictures, tool previews, 3D
views), and observed evaluation tracks the cell definitions an evaluation reads.
A notebook is then mostly a projection of a list of entry cells, each drawn as
its expression and its result, with ergonomics: a new entry on Enter, and
progress and cancellation for long computations. It's a starting point of its
own, beside Grap's.

- Entries are cells, and one refers to another the standard ways: Cmd-clicking
  it into a slot, copy and paste, or naming it and referring to it by name. No
  automatic names. The picker doesn't yet offer named cells that aren't
  functions, a gap to note rather than fix first.
- The notebook knows what it is, so an entry is its expression alone, with no
  wrapper. `{evaluate: expression}` is a poor design as it stands: a reference
  to one gets the record, not its result.

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
already keyed by identity, and list elements have saved fractional positions,
the kind of identifiers sequence CRDTs use; see list positions above.

### A common thread

CAM, printing and PCBs share a shape. Progred owns the design logic and its live
projections, and hands off to a mature tool for the last step: a CAM
post-processor, a slicer, or KiCad.

The keyboard could tie these together: a whole product, board, plate and case,
in one document with shared definitions and version control.
