# Data and editor model

This describes the implementation as of 2026-09-05. It is a reference for
working on the code, not an attribution of design intent to the owner. Earlier
models and unverified rationale are preserved in [historical notes](history/model-notes.md).
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
definition pairing a descriptive value with its Rust implementation. Reads and
calls use the same lookup. Reading a native definition uses its description;
calling it invokes its implementation. An absent or non-callable result does
not try another source. Library descriptions (currently name records) live in
that same table under the library identities, not in a separate metadata store.
Replacing a loaded library replaces all its contributions in place, including
projections and completions, before those are composed.
Explicit source-qualified paths still reach a particular definition; loaded
library values remain read-only. See [Sources](../progred/src/sources.rs),
[Library and Libraries](../progred/src/libraries/mod.rs), and
[Grap evaluation](projections.md#evaluation).

The text bridge is a temporary import/export representation. Its binders and
spelling do not add GID semantics. Native binary storage is not implemented;
see [the bridge format](gid-text.md).

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

Selection also names its owning workspace view. Two panes can show the same
stored path while keeping separate focus, scroll, and folds. A workspace
`Root` supplies this session identity; it is not a GID cell.

[`SourceTrace`](../progred/src/hover.rs) serves source-linked drawing and
highlighting. It is either a stored path or a cell, definition source, and path
relative to that definition. Normalizing to the nearest followed definition
lets different occurrences highlight the same source without conflating two
libraries' definitions of one cell. Both forms still need the caller's
resolution context. Neither is an execution stack or global address.

Computed values start the ordinary projection from a transient root. They are
read-only and attribute selection to stored source where available; the editor
does not fabricate writable paths for their children.

## Selection and editing

[`Selection`](../progred/src/selection.rs) owns a workspace root, path, GID
payload, and optional Rust editor. Its stages distinguish an existing edge,
a pending value, and a pending record label. Annotation records live in a
path-keyed [`Annotations`](../progred/src/annotations.rs) trie owned by the view;
folding is one convention in those records.

Ordinary selection does not initialize editing state. Its role is derived from
the current location: a writable missing value uses the pending picker, while
an existing value is selected normally. An explicit pending or label payload
can still request those modes. The picker renders an empty query by default and
materializes its editor only on input; query, caret, choice, scroll, and expansion
state are optional. This applies to empty roots, bare-cell definitions, missing
record fields, and inserted list positions, without a special selection callback
for each. Read-only locations do not enter a picker.

The live `LineEditState` owns text, caret, IME, and text-drag state. A projection
at the selected location receives a GID description derived from that state.
A capability replacement decodes it once into the live editor; there is no
second retained editor representation to synchronize. The path library encodes
typed paths as ordinary GID lists: `{key: cell}`, `{element: {indexable: blob}}`,
and `{follow: document}` or `{follow: {library: cell}}`. Decoding checks each
step and the canonical list-position bytes. Encoding a position does not
extend its lifetime: loading a document still regenerates its list positions.

Site and selection access are scoped foreign calls. `site path` returns the
projected site's document path; `selection get` reads the selection payload
there. `selection set` takes an explicit `path` and `value`, interpreted in
the supplied document and view, so it can move selection to another location.
An absent value clears selection only at the specified path. Reads observe
staged writes; an explicitly declined or halted handler commits no effects.
Ordinary absent results keep effects, including a setter's successful clear.
A function must decline before performing effects, including effects in its
arguments or nested calls. Declining afterward halts the evaluation and prints
an error. Only the complete editor operation is staged,
with no per-call snapshots or rollback. Tests can replace these foreign functions
with a recording interpreter.

The projection supplies a line's spelling, presentation, and conversion callback.
The current line handler owns the callback; selection retains neither a Rust
callback nor a Grap callable. Progred's [line control](../progred/src/projection/line_control.rs)
runs an editing operation, then converts only when the accepted operation changed
the text. The callback receives the live value and spelling; `None` declines a
write. An equal result does not rewrite the document. Invalid intermediate text
can remain in the editor while the last valid value remains in the document.
Loaded library values decline writes. Projections decide how unrelated fields
survive an edit. Native atomic libraries use native conversion functions; the
line library's `grap` adapter calls a Grap conversion with text and current value
as data, mapping absent to `None`.

The line control groups the first document write into undo and coalesces later
writes in that editing run. The shell no longer runs a general post-event
conversion step. Query editing similarly resets its completion choice and scroll
at the editing operation, not after unrelated events.

In the normal projection, blobs use a monospace hex line with a fixed `0x`
prefix. The buffer contains the full hex digits, including for blobs longer
than the structural summary. Edits accept complete bytes in either case;
empty hex denotes an empty blob. Query entry and editing share the blob
library's parser. Raw retains its compact structural blob display.

Navigation landmarks contain their selection callbacks. A selected, writable
line with no editing state uses its current projected spelling with the caret
at the end. Rendering and input use the same default; projection does not store
it just because the line is selected. On an editing interaction, the line control
materializes state from that default. Its current handler supplies conversion. Existing
caret, in-progress spelling, and IME state take precedence. Raw and read-only
views do not acquire an editor from a plain selection.

Ordinary selection therefore needs only a location, including after completion,
paste, deletion, and undo. Navigation supplies its movement direction to the landmark,
not the keyboard event (and no direction for direct selection):
the line control explicitly seeds the start for leftward entry, while other
entries use the default. Pointer placement also remains an explicit interaction.
The shell does not inspect values or the render tree to infer editability.
See [navigation](../progred/src/navigate.rs).

Ordinary hover, selection, and related-occurrence highlights share one padded
outline. Each occurrence paints at most one mark: selection takes precedence,
then related selection, direct hover, and related hover. Pending text frames keep
their own tighter outline through focus and editing; this widget styling does
not determine the geometry of ordinary value highlights.

Cmd+A (Ctrl+A off macOS) selects the current view's root through its navigation
callback. With no selection, it targets the document root. Focused text fields
handle the shortcut first and select their own text.

## Completion

The projection rendering a pending value or label explicitly requests
completion and may supply a lazy vocabulary. Only the active picker asks the
provider for offers. Each request includes the query, field/value kind,
suggestion/Everything scope, source-qualified path, and read-only path and cell
lookups. Cell lookup exposes the selected definition's value, source, and whether
it has a native implementation, without evaluating it.
The path names a missing value or the record receiving a new label. A local
projection provider takes precedence; otherwise library providers contribute
in library order. `None` leaves the vocabulary unspecified, while `Some([])`
means an empty narrow list with the `…` escape. If no provider specifies a
vocabulary, the editor uses its universal offers directly.

Completion display text and detail can be literal text or a named cell reference.
Library, constructor, parameter, and numeric-type labels use references; the
picker resolves their current names before filtering each frame. Renaming a
definition therefore updates even retained offers without changing their
insertion value or continuation. An explicit empty name stays empty; a missing
name uses the usual short cell identity. Typed values remain literal text.
Cell search entries show their source name as a right-aligned note, without
appending the cell identity. Identities are low-level inspection information,
not routine disambiguation labels; Raw still shows them as the primary spelling.

The completion library's `labels` helper turns cell identities into label offers;
`combine` concatenates applicable lazy providers in order, preserving an explicitly
empty vocabulary. Grap's `parameter_labels` reads an inline or stored lambda's
declared parameters when asked. The same metadata reader supplies call field order
and `call_completion`'s initial pending parameter. It follows cell aliases, declines
cycles and computed callables, and does not interpret native descriptions as lambdas.
Filtering names and excluding existing record labels remain picker responsibilities.

Root templates and root field suggestions are ordinary library providers
checking the path, not separate editor hooks. The name library offers the query
as text at a name field, including an empty string and optional surrounding
quotes. Expanding the picker still allows other values. Fidget uses the same interface
for shape expressions, parameter labels, and f32 parameter values, including
through cell references, source-list items, and existing Grap constructor calls.
Shape templates open their first missing parameter
without inventing a value. Label offers omit fields already in the record.
Fidget field-expression suggestions put shapes before scalar constants; numeric
parameter slots still lead with their f32 offer, including zero for an empty query.
Circle and sphere are ordinary named Grap lambdas that use quote/unquote to
return Fidget arithmetic; the Fidget parser has no circle or sphere forms.
Their parameter suggestions and initial focus come from their current definitions;
the native Fidget forms retain explicit domain schemas.
Their completions insert calls only in evaluated positions (the domain's source
entries and Fidget constructor arguments), not inside inert Fidget records.

Libraries can also contribute query-dependent value offers to the universal
vocabulary. The numeric libraries offer `f32`, `f64`, and `u64` interpretations
when the query parses, showing the representation and the actual stored
number. An empty or whitespace-only query offers zero in each available
representation; it remains a suggestion until committed. Other invalid numeric
queries offer no number. Universal offers use a general ordering: strong named
cell and constructor matches, library-provided interpretations in library order,
plain text (or a new label), then weak fuzzy and unnamed references. The editor
does not distinguish particular numeric representations for ranking. With an
empty query, the zero interpretations therefore follow named cells and
constructors, just before the empty string. Explicit quoted text and blob syntax
lead instead. Numeric providers decline label requests. A projection's narrow
vocabulary still takes precedence until the user expands it.

Universal constructor offers accept delimiter aliases: `[` for `new list`,
`(` for `new cell`, and `{` for `new record`. In an empty completion query,
those keys activate the constructors directly, including in a provider's narrow
list. A field label permits only the cell constructor. Nonempty queries and
IME composition keep ordinary text input; quoted punctuation leads with literal
text. Shortcuts use the same insertion callbacks as the constructor offers.

A library completion can provide an `on_commit` Grap callable, run at the
committed location with the same site and selection capabilities as event
handlers. Insertion and continuation effects are prepared together and installed
unless the callable explicitly declines or evaluation halts. Ordinary absent
results do not veto the completion. Selection changes are effectful calls, not a
special return-value format. Insertion itself does not change selection: the
continuation receives the existing selection, and only its explicit effects
replace or clear it. A low-level offer with no continuation leaves that selection
unchanged, including an active pending query. Stock offer combinators supply the
policy: `completion::select` selects the inserted value, and `completion::label`
opens the label's missing value. Text, numbers, and blobs simply select their
location; their line controls supply default editing when projected. Completions
do not copy editor text, caret, or write-back rules into the selection. Query
caret positions are not translated across parsing. These same offers work in
Raw without an override, since that projection contains no atomic line control.
Enter commits only through the placed completion control; the shell has no
fallback which inserts query text after its offers decline.

Root `grap` and `fidget` offers create a fresh bare
cell shared by their domain field and a left pane. Fidget's pane applies
`preview 3d`; Grap's pane renders the evaluated result. The continuation opens
the cell's pending document definition through the domain field, without
inventing a placeholder value. Offers that mint identities construct their
value on activation, so reusing an offer creates independent cells.
`panes` remains an independent root field suggestion.

The placed frame retains the exact visible offers. Each offer has an activation
callback plus explicit text, styling, matching spans, and source attribution.
The reusable Puri widget shapes and draws rows. Progred's
[native card widget](../progred/src/display/widget/completion.rs) composes navigation,
interaction, and the shared scroll container. Its caller supplies state and
activation callbacks; the widget knows nothing about paths, insertion, or Grap.
The editor owns document operations, query state, and popup placement. See
[offer construction](../progred/src/completion.rs) and
[document adaptation](../progred/src/projection/completion.rs).
The card meets the query's painted frame, accounting for the frame outline and
both border widths. Placement uses those same drawing parameters above or below
the query, and includes the card's stroke when keeping it within the viewport.

A provider starts with its narrow vocabulary. The trailing `…` participates in
row navigation but activates expansion rather than committing a value. Expansion
also happens when pressing Down on the selected `…` row. Expansion keeps the
selected index, clamped to the available rows. Changing the query filters the
expanded list and resets selection and scroll; expansion lasts for that picker.
Returning to an older query does not restore an old choice. A new picker starts
with its provider's narrow vocabulary. Pointer and keyboard activation use the
same callbacks.
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
{panes: {left: [...values...], right: [...values...]}, ...}
```

The lists determine side and order; entries are ordinary values. Deleting a
pane's root removes that list element. Opening and moving panes edit these
lists, so they save and undo with the document. A malformed pane container or
nonrecord root is not overwritten to make the operation possible.

Pane identity, projection mode, folds, scroll, and requested size are session
state. The document view starts with the contents of `panes` folded; individual
pane entries have no initial fold. Expanding the field is preserved when panes
are added, removed, or moved. Surviving declarations retain their view state.
Moving an entry assigns a new list position and explicitly retargets selection;
its size, scroll, and projection mode carry across, while path-keyed folds reset.

`{value: source, projection: function}` is a presentation-library convention,
not workspace configuration. The document's normal projection shows this
declaration as editable data. At pane entry, including through cell definitions,
the view tries the presentation library's declaration interpreter first. Nested
values and computed results use the normal projection, so declarations inside
them remain data. Raw shows the structural data in either view. See
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

The IoP example uses an inline Grap viewport function to construct a drawing
program with the assigned width and height. Those dimensions also reach `tree
scene`; its drawing units and editable tree parameters are not rescaled.

## History, gestures, and persistence

Examples and replace-in-place New Document are development/demo conveniences,
not the intended production File menu. Cmd+N (Ctrl+N in the drawn menu) and the
example shortcuts replace the current document after confirming any unsaved
changes. With no desktop window, they create one. Cmd+Shift+N / Ctrl+Shift+N
is New Window; Open also continues to create a separate window.

Replacement keeps the window, surface, geometry, fonts, text-shaping cache,
clipboard, library stack, physical pointer/modifier input, and debug-display
preference. It resets document/saved identity, history, selection and text/IME
state, all pane identities and annotations (including camera and folds), scroll,
projection modes, divider state, binders, menus, gestures, queued input, retained
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
scroll positions, projection modes, and unrelated annotations alone in surviving
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
directory. Browser and iOS capabilities differ; see
[persistence](../progred/src/text_store.rs) and [platform notes](platforms.md).

Frame construction, dispatch order, hover, and clipping are described in
[Puri and the editor frame](puri.md).
