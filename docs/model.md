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
[Library and Libraries](../libraries/src/lib.rs), and
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

The projection supplies a line's spelling, presentation, and Grap write-back
function. The function receives the current value and input text as data.
A non-absent changed result writes through to the stored location; an absent
leaves the document alone. Invalid intermediate text can therefore remain in
the editor while the last valid value remains in the document. Loaded library
values decline writes. Projections decide how unrelated fields survive an edit.

In the normal projection, blobs use a monospace hex line with a fixed `0x`
prefix. The buffer contains the full hex digits, including for blobs longer
than the structural summary. Edits accept complete bytes in either case;
empty hex denotes an empty blob. Query entry and editing share the blob
library's parser. Raw retains its compact structural blob display.

Navigation landmarks contain their selection callbacks. Moving onto an
editable line installs the description produced by that projection, including
the intended caret position. The shell does not inspect the render tree to
infer editability. See [navigation](../progred/src/navigate.rs).

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
checking the path, not separate editor hooks. Fidget uses the same interface
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
special return-value format. Root `grap` and `fidget` offers create a fresh bare
cell shared by their domain field and a left pane. Fidget's pane applies
`preview 3d`; Grap's pane renders the evaluated result. The continuation opens
the cell's pending document definition through the domain field, without
inventing a placeholder value. Offers that mint identities construct their
value on activation, so reusing an offer creates independent cells.
`panes` remains an independent root field suggestion.

The placed frame retains the exact visible offers. Each offer has an activation
callback plus explicit text, styling, matching spans, and source attribution.
The reusable Puri widget draws rows; Progred owns the document operations,
query state, scrolling, and floating card. See [offer construction](../progred/src/completion.rs)
and [completion presentation](../progred/src/projection/completion.rs).

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

## History, gestures, and persistence

Examples are a development and demo aid, intended to be removed from production
builds. On desktop, they open through the normal new-window path, then close the
previous editor if it has no unsaved changes. This includes the empty startup
document and untouched examples; edited documents stay open. File New and Open
continue to create windows without closing the previous editor.

[`History`](../progred/src/history.rs) keeps document snapshots and selection
paths in undo/redo stacks. Recording a new branch clears redo. The saved mark
is a position on that surviving branch: discarding the branch that held it
clears the mark, so reaching the same stack depth cannot report a different
state as saved.

A line's first write records its undo step; subsequent writes in that edit run
coalesce. Saving breaks the run. Projection gestures have one caller-owned
continuation in [`gesture`](../progred/src/gesture.rs). The accepting handler
starts it; the continuation owns domain updates, undo grouping, release, and
cancellation. Replacing/restoring the document or saving ends it.

The platform supplies persistence and clipboard capabilities. macOS uses its
native atomic write API. Linux writes a unique sibling temporary file,
synchronizes it, renames it over the destination, and synchronizes the parent
directory. Browser and iOS capabilities differ; see
[persistence](../progred/src/text_store.rs) and [platform notes](platforms.md).

Frame construction, dispatch order, hover, and clipping are described in
[Puri and the editor frame](puri.md).
