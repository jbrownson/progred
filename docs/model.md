# Data and editor model

Earlier models and unverified rationale are preserved in
[historical notes](history/model-notes.md).
Open work is listed separately in [deferred work](deferred.md).

## Data and resolution

[GID](gid.md) defines `Value` as cell references, blobs, lists, and records.
A document has an optional root and a `CellId -> Value` table. An identity with
no table entry is a bare cell. Record labels are cell identities; text, names,
numbers, absence reasons, and domain data are library conventions.

A cell may accidentally occur in several sources, for example when editing a
document that also exists as a loaded library. Normal lookup selects the document
definition first, otherwise the first loaded library definition. Names, structural
display, and cell evaluation use that value. Definitions are neither merged nor
implicitly composed. Duplicate indicators and inspection UI are deferred.

Each library has one definition per cell: either an ordinary value or a native
definition (`grap::Definition::Foreign`) pairing a descriptive value with its
Rust implementation. Reads and
calls use the same lookup. Reading a native definition uses its description;
calling it invokes its implementation. An absent or non-callable result does
not try another source. Library descriptions (currently name records) live in
that same table under the library identities, not in a separate metadata store.
Replacing a loaded library replaces all its contributions in place, including
projections and completions, before those are composed.
Explicit source-qualified paths still reach a particular definition; loaded
library values remain read-only. See [Sources](../progred/src/sources.rs),
[Library and Libraries](../progred/src/library.rs), and
[Grap evaluation](projections.md#evaluation).

Documents are saved and loaded through the [text bridge](gid.md#text-bridge).

## Occurrences, definitions, and views

A `Path` is a sequence of GID steps interpreted in a supplied document and
library context:

- `Key(cell)` enters a record field.
- `Element(position)` enters a list element at its stable position.
- `Follow(Document | Library(id))` crosses a cell into a particular definition.

The empty path is the document root; it need not be a cell. Library sources use
stable identities, so changing load order cannot silently retarget a `Follow`.
List positions preserve an occurrence across edits that retain that position;
they are not content identities or globally meaningful addresses.

A [`Location`](../progred/src/location.rs) names stored data: it starts at the
document's root or at a cell's definition from a particular source, then follows
a path from there. Following a cell starts a new location at its definition,
however the cell was reached, so what can be edited depends on where the data is
stored, not on the route to it. Writes land in that stored value: a location's
last `Follow` names the definition it is in, or else its own start does. The
root and the document's own definitions are writable, including a cell with no
definition yet; a library's definitions are read-only. A route through a cell
and a location starting at that cell are the same location.

Selection also names its owning workspace view. Two panes can show the same
stored path while keeping separate focus, scroll, and folds. A workspace
`Root` supplies this session identity; it is not a GID cell.

[`SourceTrace`](../progred/src/display/widget/source.rs) serves source-linked
widgets, drawing, and highlighting. It is a location in the form it is stored
in: a path from the root, or a cell, definition source, and path relative to
that definition. Normalizing to the nearest followed definition lets different
occurrences highlight the same source without conflating two libraries'
definitions of one cell. Both forms still need the caller's resolution context.
Neither is an execution stack or global address.

Computed values use `at` at their own displayed occurrences. They and their
children can be selected, copied, and folded, but have no document destination
for edits. Selecting a result does not select its producing expression. A cell
inside a result is still a cell, though, as in a missing-cell error or a quote's
template: following it starts a location at its definition, which edits on the
usual terms. So the missing-cell error a pane shows for a Grap starting point's
unwritten program is a place to write it. Values copied out of cells stay
read-only; reaching the code that made one is source-linked hover's job.
Computed output uses the same read-only ground as external definitions. Nested
read-only occurrences do not stack washes; an explicit jump to writable source
restores the paper ground. This decoration does not change edit permissions:
those still follow the occurrence's source/conject. A reference to an external
definition can itself still be replaced or removed from a writable document.

## Selection and editing

[`Selection`](../progred/src/selection.rs) owns a workspace root, path, GID
payload, and optional Rust editor. Its stages distinguish an existing edge,
a pending value, and a pending record label. Annotation records live in a
path-keyed [`Annotations`](../progred/src/annotations.rs) map owned by the view;
folding is one convention in those records. A projection folds what it draws by
consulting that state through [`display::fold`](../progred/src/display/fold.rs),
and decides how it starts: Raw's cells, lists and records fold, as do Grap's
calls and the control forms. Space folds and unfolds the selected occurrence
where its projection folds. Clicking a fold's `…` selects the occurrence as it
opens it, so Space closes it again.

Raw's lists and records start open. A cell starts closed where drawing it would
draw its value again inside itself, a cycle, and where it sits inside another
cell's value and holds cells itself, since opening it could open more in turn.
The cells a view reaches directly open, so their projections get going, and so
do cells holding no cells, which can't cascade. A cell whose whole value is
another cell opens through to it.

Selection also retains a native editing scope, which maps an occurrence to its
location, if any. Ordinary document projection uses its allocation-free identity
case. A scope installs a **conject** at an occurrence: a location whose path
the remaining projection path continues from that location's start, as a jump
installs, or no location at all, as a computed value has. Reads and writes use
the same conject; there is no independently configurable read/write pair or
inverse mapping. Several
occurrences may conject to one source. Selection, navigation, and annotations
stay occurrence-local, while source-linked hover uses the conject. Line editing,
gestures, completion insertion, structural paste/deletion, and history retain
that interpretation. Undo restores the scope with the selected path; scopes
contain no document snapshot or borrow of the editor.
A scope is a chain of layers, each reading the occurrences at and beneath one
occurrence: a conject's, or a followed cell's definition. Projection adds a
cell's layer wherever it follows a cell, which is how a location starts there.
Identity scopes borrow their input path without allocating. Comparing two
scopes at an occurrence compares where each says the data is stored.
If a projection changes what a selected occurrence means, its caller is
responsible for clearing the selection when necessary; the editor does not
try to repair arbitrary changes of interpretation.

`descend` extends the current occurrence, `jump` gives a new occurrence an actual
document source, and `at` projects a supplied value without a source. `None`
means no document location, not a missing value: `Some(location)` can name an
absent field and still offer editing. A library source is present but read-only.
Ordinary descendants of `at` stay detached until they follow a cell, which starts
a location at its definition; an explicit `jump` can also establish a source
again. Outline list entries project their referenced
fields through jumps: the heading is the real list element, and its body has an
occurrence beneath that element. Repeating a field yields independent folds and
selection locations while both bodies edit the same source field.

Copy and fold do not resolve an occurrence through conject. The shared
projection boundary installs handlers that copy the displayed `Value`, and a
folding projection changes the occurrence's fold annotation using the default
it determined. This also works for source-less `at` values. Inner widget
handlers take precedence (for example, copying a text selection). Structural
cut copies that same value, then attempts deletion through the selection's
conject; a source-less value can be copied but not deleted.

Ordinary selection does not initialize editing state. Its role is derived from
the current location: a writable missing value uses the pending picker, while
an existing value is selected normally. An explicit pending or label payload
can still request those modes intentionally, but no caller needs one as setup.
The picker renders an empty query by default without writing selection state and
materializes its editor only on input; query, caret, choice, and scroll state
are optional. This applies to empty roots, bare-cell definitions, missing
record fields, and inserted list positions, without a special selection callback
for each. Read-only locations do not enter a picker.

Enter beside a selected value adds a peer: an element after it in a list, or a
field on the record holding it. Cmd+Enter adds something within it: a field on
a record, an element appended to a list, or a bare cell's first value. Both
belong to the view of the container they land in, which offers them through
`structure::authoring`; its children's handlers run first. The structural
record, list and cell views offer it. Grap's calls don't, since their arguments
are their parameters, shown as slots when missing; nor does a view that draws a
value as one piece, such as a number, text, a color, an infix expression or a
tutorial's slots, so Enter there adds nothing. While typing in a
value, Enter only ends the typing, since the value is already written; the
text re-spells from it, so a half-typed `6e` goes back to `6`. A key a line
editor doesn't take never starts its typing.

Deleting a selected value keeps its place selected as a hole, so the next
keystrokes write its replacement; Backspace or Delete on the hole's empty query
removes it, Backspace landing on the sibling before it, else on its container,
and Delete on the stop after it, even past its container's end. Typing over a
selected value opens a picker in its place seeded with what was typed: an
explicit pending payload over the existing value, which projection draws as
missing until a choice commits, so the document is untouched until then. Where Grap is
shown, one of a number type's single-character operators typed at the end of a
selected number instead makes the number that operator's left operand and opens the
right operand's picker; a sign or exponent the number is still becoming stays
in its text. A picker opened from a key remembers where it was opened, and Escape, or an empty
query's Backspace or Delete, steps back there. Views that draw a record as one
value (numbers, text, colors) follow it with a record of its other fields, as a
call follows its head with its arguments. No key adds a field to them there; a
rawer view, where the value reads as the record it is, can.

The live `LineEditState` owns text, caret, IME, and text-drag state. A projection
at the selected location receives a GID description derived from that state.
A capability replacement decodes it once into the live editor; there is no
second retained editor representation to synchronize. Paths reach Grap as
[ordinary GID data](gid.md).

Generic Grap handlers receive GID events and caller-supplied capabilities.
Site and selection access are scoped foreign calls. `site path` returns the
site's occurrence path in its view; `selection get` reads the selection payload
there. `selection set` takes an explicit `path` and `value`, interpreted in
the supplied document and view, so it can move selection to another location.
An absent value clears selection only at the specified path. Reads observe
staged writes; an explicitly declined or halted handler commits no effects.
Ordinary absent results keep preceding effects. Setters return `{}` on successful
set or clear; absent remains an input convention for clearing, not its return value.
Site and selection getters return their own missing-state reasons when empty.
A function must decline before performing effects, including effects in its
arguments or nested calls. Declining afterward halts the evaluation. Only the
complete editor operation is staged,
with no per-call snapshots or rollback. Tests can replace these foreign functions
with a recording interpreter.

The projection supplies a line's spelling, presentation, and conversion callback.
The current line editor's handler owns the callback; selection retains neither a
Rust callback nor a Grap callable. The line editor, `display::line_edit`, is the
stock native widget function carried by `Layout::widget`; layout has no
line-editor constructor or interpreter
arm. The editor supplies current state and scoped editing capabilities, then
incorporates the widget's render closures, handlers, hover claim, and navigation
transition. Progred's [line edit](../progred/src/editing.rs)
runs an editing operation, then converts only when the accepted operation changed
the text. The callback receives the live value and spelling; `None` declines a
write. An equal result does not rewrite the document. Invalid intermediate text
can remain in the editor while the last valid value remains in the document.
Loaded library values decline writes. Projections decide how unrelated fields
survive an edit. Atomic libraries supply native conversion functions.

Query editing resets its completion choice and scroll at the editing operation,
not after unrelated events.

Raw draws a blob as a monospace hex line with a fixed `0x` prefix, edited in
place; no library draws blobs. One of more than 16 bytes shows its first eight
bytes and its length until it's selected, and one of more than 1024 stays that
way, replaced by typing over it rather than edited. Edits accept complete
bytes; empty hex denotes an empty blob. Query entry and editing share GID's hex
reader, `gid::read_hex`.

A selected, writable
line with no editing state uses its current projected spelling with the caret
at the end. Rendering and input use the same default; projection does not store
it just because the line is selected. On an editing interaction, the line editor
materializes state from that default. Its current handler supplies conversion. Existing
caret, in-progress spelling, and IME state take precedence. Read-only views,
and Raw outside a blob, do not acquire an editor from a plain selection.

Ordinary selection therefore needs only a location, including after completion,
paste, deletion, and undo. Navigation targets receive an optional arrival
direction. The line editor places the caret at the beginning when moving Right
into it and at the end when moving Left; ordinary and vertical arrivals keep
the default. This policy belongs to the widget, not navigation dispatch. Pointer
placement remains an explicit interaction. The shell does not inspect values
or the render tree to infer editability. See
[`Select`](../progred/src/display/widget/navigation.rs) and
[the line editor](../progred/src/display/widget/line.rs).

Arrow keys move the selection between stops that projections declare, in an
order the chosen layout determines; [layout navigation](navigation.md) is the
reference for how stops are declared, ordered, and dispatched. Destinations
keep the existing occurrence path and editing or conject context, and
navigation never resolves document paths to choose one.

Line editing uses the host-supplied keyboard convention: Mac Command+Left/Right
moves to the content's ends and Option+Left/Right moves by words. Control-based
hosts use Control+Left/Right for words. Shift extends the selection in each case.
These controls handle movement before structural navigation; affixes remain
outside the editable span.

Ordinary hover, selection, and related-occurrence highlights share one padded
outline. Each occurrence paints at most one mark: selection takes precedence,
then related selection, direct hover, and related hover. Pending text frames keep
their own tighter outline through focus and editing; this widget styling does
not determine the geometry of ordinary value highlights.

The editor owns an explicit `Palette` input, used by text, selections, library
grounds, menus, completion cards, and popovers. `Theme::Light` uses white paper,
blue names, violet field labels, and orange literals. `Theme::Dark` supplies
brighter corresponding colors on navy paper. The macOS shell follows the system
appearance at startup and on changes. Its native View menu offers Follow System
Appearance, Light Mode, and Dark Mode; this application-wide override lasts for
the session, including newly opened windows, and also updates window chrome.
Other hosts default to Light; the browser host can explicitly choose either theme.
Read-only projection boundaries use a faint tint; returning to writable source
restores the same opaque paper used by the window. Changing the
palette rebuilds the ordinary complete frame without replacing the document,
selection, or undo history. Authored drawing colors remain document data, not
theme colors. These styles change presentation, not projection recognition or
source/selection policy.

Cmd+A (Ctrl+A on non-Mac hosts) selects the current view's root through its
landmark's direct-selection callback. With no selection, it targets the document root. Focused text fields
handle the shortcut first and select their own text.

## Completion

The projection drawing a hole decides what it offers. A hole is a pending value
or label; its projection draws a [picker](../progred/src/display/widget/picker.rs)
there, an ordinary toolkit widget, and gives it the hole's offers. A
projection's own offers, ranked against the query, are then the hole's whole
vocabulary: the picker shows those offers and nothing else. The generic
structure projection's holes, and the hole the editor draws for a missing value
no projection draws, take Raw's picker. Raw offers the editor's constructors and what's typed, read as a value: what the
libraries read the query as, such as a number, then text, or for a field the
name library's new label named as typed, and bytes typed in GID's own `0x`
spelling. It offers no existing cells. An existing cell
is reused by pointing at it (Cmd+click) or pasting it; typing makes something
new. Cmd+clicking a field's label points at the label, not the field's value. The hole at a document's root starts the document: it leads with what the
libraries switched on there start one as, such as a Grap program or a Fidget
scene, then offers Raw's. The Raw view switches every library off, so its holes
offer only GID's own kinds: the constructors and `0x` bytes. A picker can also take keys that choose an offer at once
from an empty query, as Raw's brackets do. No provider is hidden in traversal
context. Only the active picker asks for offers.

[Deferred work](deferred.md#completion-after-the-strip-back) lists what
completion may add.

Each request includes the query, field/value kind, the question asked of a
library (what it offers anywhere, what it reads the query as, or what a document
can start as), the hole's path, read-only path and cell lookups, lazy enumeration
of defined cells, the document's names for cells, and the loaded libraries'
provider. Cell lookup exposes the selected definition's
value, source, and whether it has a native implementation, without evaluating
it. The path names a missing value or the record receiving a new label, from
where the hole's location starts, and path lookups read from there too. A hole
in a cell's definition reads that definition as its own context, however the
cell was reached, so Grap offers only the bindings visible within it. A
projection's provider that answers `None` offers nothing, as `Some([])` does.
Raw asks the libraries for their readings of the query, a document's root for
their starting points, and a projection's provider for whatever it wants, as
Grap's slots ask for their offers anywhere. They answer in library order; `None`
means a library has nothing to add.

Completion display text and detail can be literal text or a named cell reference.
Library, constructor, parameter, and numeric-type labels use references; the
picker resolves their current names before filtering each frame. Renaming a
definition therefore updates even retained offers without changing their
activation handler. An explicit empty name stays empty; a missing
name uses the usual short cell identity. Typed values remain literal text.

The completion library's `combine` concatenates applicable lazy providers in
order, preserving an explicitly empty vocabulary. Grap's parameter reader takes
an inline or stored lambda's declared parameters when asked, for a call's field
order, its slots for missing arguments, and `call_completion`'s initial pending
parameter. It follows cell aliases and declines cycles and computed callables. A
native function's description lists the arguments its implementation requires
under `params`; the reader takes that list as is and never interprets a native
description as a lambda. Filtering names and excluding existing record labels
remain picker responsibilities.

Asked for its offers anywhere with a nonempty query, Grap offers calls to named
stored lambdas,
closures, and native functions discovered through those definitions. These are
ordinary `call_completion` offers, distinguished by a `call` detail; choosing one
inserts `{function: cell}` and opens its first declared argument when known.
Call suggestions put document-defined functions before library functions, sorting
each group alphabetically by display name, ignoring case. Typed-query matching
still takes precedence; this order breaks equally ranked matches.
Discovery does not evaluate code or guess the result of computed callables.
Label pickers do not receive these call offers.

Grap expression slots supply their own provider: statically visible lambda
parameters, preceding `let`/`where` bindings and the current match case's pattern
bindings, plus named function calls, every library's offers anywhere, the editor's cell,
list, and record constructors, quote, and what's typed, read as a value. This is an explicit local
projection, including the items of `do`, not a global change to data completion.
Binding suggestions put the nearest enclosing lexical scope first, then outer
scopes, sorting names alphabetically within each scope. Rebinding a cell gives it
the nearer scope's rank; scrolling and layout have no effect on this order.
Typed-query match quality still ranks ahead of proximity. The request carries
the loaded libraries' provider for composition; Grap reuses those offers rather
than duplicating numeric conventions. Scope discovery reads
source structure only; quoted template declarations do not introduce bindings
into unquotes. Computed binding lists and runtime callable types are not
inferred. Cells that are neither bindings nor functions, such as a named
constant, are not offered there.

A projection that descends into a name passes the name library's partial, which
draws an existing name as its line editor and a missing one as a hole taking
whatever is typed as its text, including an empty string and optional
surrounding quotes. A missing-name marker, such as an anonymous lambda's,
selects that name location and then draws the same hole. A `name` field drawn by
the generic structure projection offers Raw's instead. Circle and sphere are
ordinary named Grap lambdas that use quote/unquote to return Fidget arithmetic;
the Fidget parser has no circle or sphere forms.

The numeric libraries read the query as `f32`, `f64`, and `u64` numbers when it
parses, showing the representation and the actual stored number; a query that
doesn't parse, an empty one included, reads as no number. Any nonempty query
reads as text, after the readings particular to it; the empty string takes at
least a `"`. A new label likewise needs something typed. An empty query
therefore offers no typed values, and Raw shows just its constructors, after a
document root's starting points. A projection whose hole expects a number or
text can still offer one for an empty query, as a name's hole does for text.
Raw orders its offers: a reading that spells the query exactly, the
constructors the query names, the other readings in library order, text (or a
new label), then constructors the query matches only loosely. The editor does
not distinguish particular numeric representations for ranking. Explicit quoted
text and blob syntax lead instead. Numeric providers decline label requests.

Raw's holes, and a document's root, make the editor's constructors from their
delimiters: typed into an empty query, `(` makes a new cell, `[` a new list, and `{` a new record; a field
label permits only the cell. These are keys of Raw's holes, not of the offers.
A projection's hole takes a delimiter as text, which finds its constructor where
the projection offers one, as Grap's do. Nonempty queries and IME composition
keep ordinary text input; quoted punctuation leads with literal text.
A new cell, list or record opens as it's made, as Cmd+Enter would open it,
so typing goes inside: the cell's value, an element of the list, or a field of
the record. Escape steps back out to it.

A library completion consists of presentation metadata and an activation handler
receiving the dispatch's `Session`. Clicking or pressing Enter runs it;
highlighting or moving between suggestions does not. Handlers may edit, change
selection, or start an interaction without inserting data. An optional
advertised value supplies source/filter metadata only; it never determines
activation behavior.

The completion library's `insert` and `generated` helpers build ordinary insertion
handlers. They accept an optional `on_commit` continuation, run at the committed
location against the staged selection and annotation state. `site::grap` adapts
a Grap callable to that continuation interface. Insertion
and continuation effects are prepared together and installed only if the
continuation accepts; Grap explicitly declining or halting declines the whole
operation. Ordinary absent results do not veto it. Selection changes are
effectful calls, not a special return-value format. Insertion itself does not change selection: the
continuation receives the existing selection, and only its explicit effects
replace or clear it. A low-level offer with no continuation leaves that selection
unchanged, including an active pending query. Stock offer combinators supply the
policy: `completion::select` selects the inserted value, and an insertion
continuing with `pending_at` opens a label's missing value. Text, numbers, and blobs simply select their
location; their line editors supply default editing when projected. Completions
do not copy editor text, caret, or write-back rules into the selection. Query
caret positions are not translated across parsing. These same offers work in
Raw without an override, since that projection contains no atomic line editor.
Enter commits only through the placed completion control; the shell has no
fallback which inserts query text after its offers decline.

The color library's `new color` offer changes selection to picker state at the
same missing location. No color is inserted and no undo entry is made until the
first picker interaction. List insertion positions remain stable through that
transition. The existing point-control gesture writes the selected color and
groups the drag into one undo step. Escape before choosing returns to completion;
leaving the location discards the picker without an edit. The seed color belongs
only to the picker. An area with the color library switched off, Raw among
them, offers none of the library's offers, this one included.

Root `grap` and `fidget` offers create a fresh bare
cell shared by their domain field and a left pane. Fidget's pane applies
`preview 3d`; Grap's pane renders the evaluated result. The continuation opens
the cell's pending document definition through the domain field, without
inventing a placeholder value. Offers that mint identities construct their
value on activation, so reusing an offer creates independent cells. Both are
their libraries' starting points, which lead the hole of a document's root.

The placed frame retains the exact visible offers, so hover and commit never
reconstruct a different list. Each offer has an activation
callback plus explicit text, styling, matching spans, and source attribution.
The reusable `puri-widgets` completion widget shapes and draws rows. Progred's
[native card widget](../progred/src/display/widget/completion.rs) composes navigation,
interaction, and the shared column and scroll container, and places the card
against its query; the row widget has no scroll viewport of its own. Its caller
supplies state and activation callbacks; the widget knows nothing about paths,
insertion, or Grap. The editor owns document operations and query state. See
[ranking](../progred/src/display/widget/offers.rs),
[Raw's offers](../progred/src/display/raw.rs),
[committing a choice](../progred/src/completion.rs), and
[the picker](../progred/src/display/widget/picker.rs).
The card meets the query's painted frame, accounting for the frame outline and
both border widths. Placement uses those same drawing parameters above or below
the query, and includes the card's stroke when keeping it within the viewport.
The card shows up to eight rows and scrolls the rest. It opens below the query
when it fits there, otherwise above; when it fits neither, it takes the roomier
side, showing only the rows that fit, rather than covering the query.

Changing the query filters the offers and resets selection and scroll.
Returning to an older query does not restore an old choice. Down stops at the
last row. Pointer and keyboard activation use the same callbacks.
Keyboard navigation and unpressed mouse motion over a visible row update the
same chosen-row state. Only that row is highlighted, and Enter activates it.
Hover remains available for pointer activation but does not paint a second
highlight or overwrite the choice during a redraw. Touch motion and active
mouse drags do not choose rows, so scrolling and text selection can keep their
gestures.
Activating a visible offer consumes the input even if its continuation declines;
Enter must not then fall through to inserting the raw query.

## Panes

[`workspace`](../progred/src/workspace.rs) recognizes a direct, coherent root
record convention:

```text
{main: value, panes: {left: [...values...], right: [...values...]}, ...}
```

While a view shows the editor library, a view whose value is a workspace
record starts drawing at its `main` value, so the workspace's own fields stay
out of sight. The record itself is never drawn, so nothing selects it, and
selection and navigation reach only what's inside. Plain and Raw leave the
editor library off and show the whole record, as does turning the editor
library off in the Projections menu. A root without `main` draws whole, as any
other document does; one holding only the workspace's fields, as after its
main value was deleted, draws an empty `main`. A pane showing a workspace
record starts at its main value the same way. A document
needs the wrapping only once something adds a workspace field: opening a pane
on a root without them makes the root the `main` value of a new workspace
record, moving the document view's folds with it, and starting points that open
a pane begin that way.

The pane lists determine side and order; entries are ordinary values. An entry
can name a [location](#data-and-resolution) as data: `{steps: [...]}` from the
document's root, or `{in: cell, follow: source, steps: [...]}` inside a cell's
definition, with the steps and source in the path convention's encoding. While
its pane shows the editor library, the pane draws and edits that place, so it
stays in sync with the document; Plain and Raw show the entry itself. Open
Value on Left and Right open the selection's place, starting at its nearest
cell, so a cell is a location with no steps; a computed value has no place and
opens as a copy. A place that's only empty, as after its value was deleted,
draws as any empty place does, ready to write; one that can't be reached, as
after its container was deleted, draws a read-only `place not found` absent
carrying the location. Show Panes opens a location pane on the panes
declaration, `{steps: [panes]}`, or selects the one already open, so the panes
can be edited as data while they show. Close Pane removes the selected pane's
entry. Deleting a value
pane's root does too, but a location pane's root is the place it shows, so
deleting it deletes that content. Opening, closing and moving panes edit these
lists, so they save and undo with the document. A malformed pane container or
nonrecord root is not overwritten to make the operation possible.

Pane identity, the libraries shown, folds, scroll, and requested size are session
state. The document view starts with the contents of `panes` folded; individual
pane entries have no initial fold. Expanding the field is preserved when panes
are added, removed, or moved. Surviving declarations retain their view state.
Moving an entry assigns a new list position and explicitly retargets selection;
its size, scroll, and the libraries it shows carry across, while path-keyed folds
reset.

`{value: source, projection: function}` is a presentation-library convention,
not workspace configuration. The document's normal projection shows this
declaration as editable data. At pane entry, including through cell definitions,
the view tries the presentation library's declaration interpreter first. Nested
values, computed results, and the source shown after an absent result use the
normal projection, so declarations inside them remain data. This pane-entry
following is separate from explicit local or scoped projection composition.
Raw shows the structural data in either view. See
[projection composition](projections.md#projection-composition).

An assigned-size pane uses `{value: source, viewport: function}` instead. The
function receives `value`, `width`, and `height` (logical display units) and
returns ordinary display content, including handlers. Its pane has no automatic
padding or document scrolling; the assigned rectangle clips its output. Pane
splitting determines that rectangle before the function runs, so there is no
feedback from content measurement to pane sizing. View annotations and selection
keep their existing per-view ownership. Raw returns to ordinary scrolling source
display. The viewport convention applies only at pane entry, including through
cell aliases; the main document and nested declarations remain editable data.

The Fidget example and template use this contract. Preview image dimensions are
explicit arguments, separate from the Fidget field and camera volume. Raster
resolution follows the display scale; rectangular 3D views preserve square
pixels rather than stretching the geometry. Ordinary preview calls without size
arguments retain the 256-point default.

`preview 3d` accepts either one Fidget field (white) or a scene value:
`{scene: [{field: sdf, color: rgb}, ...]}`. A scene groups separately colored
objects; it is not a CSG operator. Geometry is depth-merged before lighting,
and equal depths keep the earlier object. Colors use the existing color
library's RGB/RGBA values; omitted colors are white and alpha must be opaque.
An empty scene is transparent. Fidget's 16-bit object index limits one scene to
65,536 objects. Scene data is inert like other records: Grap code constructs
it with ordinary quote/unquote, not implicit evaluation inside the scene.
The native renderer uses Fidget's GPU color pass; the CPU/web renderer shades
the winning geometry with the same object's color. Their lighting differs.

The IoP example uses an inline Grap viewport function to construct a drawing
program with the assigned width and height. Those dimensions also reach `tree
scene`; its drawing units and editable tree parameters are not rescaled.

## History, gestures, and persistence

The drawn menu contributes its keyboard handler along with its bar and popups.
Frame composition places that handler ahead of content handlers; omitting the
menu also omits its shortcuts and keyboard navigation. Undo/redo have a separate
editing handler, so embedded tutorials retain history without inheriting Raw,
example switching, or pane commands. Native menus keep their own command routing.

Examples and replace-in-place New Document are development/demo conveniences,
not the intended production File menu. Cmd+N (Ctrl+N on non-Mac hosts) and the
example shortcuts replace the current document after confirming any unsaved
changes. With no desktop window, they create one. Cmd+Shift+N / Ctrl+Shift+N
is New Window; Open creates a separate window.

Replacement keeps the window, surface, geometry, fonts, text-shaping cache,
clipboard, library stack, physical pointer/modifier input, and debug-display
preference. It resets document/saved identity, history, selection and text/IME
state, all pane identities and annotations (including camera and folds), scroll,
shown libraries, divider state, binders, menus, gestures, queued input, retained
handlers/paint, and hover attribution. The successor frame is built immediately.
On macOS the old file's frame-autosave name is detached without deleting its
saved geometry; the represented file, title, and edited flag are refreshed.
These development commands are currently present in optimized `make dev` builds
too; optimization level is not a distribution feature flag.

[`History`](../progred/src/history.rs) is a generic pair of snapshot stacks;
recording a new branch clears redo. The editor's snapshot contains its shared
document, view-qualified selection, and per-view folds. Collapse and expand
record steps without editing the document. Undo restores fold overrides and
their view identities, including when a deleted pane returns; it leaves current
scroll positions, shown libraries, and unrelated annotations alone in surviving
views. Selection paths include the owning view rather than borrowing whichever
pane happens to be selected when Undo is invoked.

The editor holds its current and saved documents as `Rc<Document>`. Dirty state
is pointer inequality, constant-time at any document size. Successful writes use
copy-on-write; rejected writes do not detach the snapshot. Undoing to the saved
snapshot clears the dirty flag, while manually recreating equal contents does
not. Folding never changes document identity, and branch depth has no bearing on
saved state. The GID document and its on-disk representation remain unchanged.

A line's first write records its undo step; subsequent writes in that edit run
coalesce. Saving or recording a fold breaks the run. Projection gestures have one
caller-owned slot in [`gesture`](../progred/src/gesture.rs). An ordinary native
widget's accepting handler supplies the continuation; the slot does not
distinguish number, camera, or color controls. Widgets process pointer samples,
closing over document-edit runs or view-annotation setters supplied by the
editor. An edit run groups its successful writes into one undo step. Creating
the run, moving the caret, and writing an equal value do not write the document.
Release, cancellation, replacing/restoring the document, saving, or recording
a fold ends the gesture.

The platform supplies persistence and clipboard capabilities. macOS uses its
native atomic write API. Linux writes a unique sibling temporary file,
synchronizes it, renames it over the destination, and synchronizes the parent
directory. Browser capabilities differ; see
[persistence](../progred/src/text_store.rs) and [platform notes](platforms.md).

Frame construction, dispatch order, hover, and clipping are described in
[Puri and the editor frame](puri.md).
