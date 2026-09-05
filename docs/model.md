# Data and editor model

This describes the implementation as of 2026-09-04. It is a reference for
working on the code, not an attribution of design intent to the owner. Earlier
models and unverified rationale are preserved in [historical notes](history/model-notes.md).
Open work is listed separately in [deferred work](deferred.md).

## Data and resolution

[GID](gid.md) defines `Value` as cell references, blobs, lists, and records.
A document has an optional root and a `CellId -> Value` table. An identity with
no table entry is a bare cell. Record labels are cell identities; text, names,
numbers, absence reasons, and domain data are library conventions.

A cell can have a document definition and definitions from several loaded
libraries. The document contributes at most one value; each library contributes
at most one value and potentially several foreign implementations per cell.
The host preserves library identity and definition order. It does not merge
these definitions into one value. See [Sources](../progred/src/sources.rs),
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
second retained editor representation to synchronize. Paths remain in Rust.

The projection supplies a line's spelling, presentation, and Grap write-back
function. The function receives the current value and input text as data.
A non-absent changed result writes through to the stored location; an absent
leaves the document alone. Invalid intermediate text can therefore remain in
the editor while the last valid value remains in the document. Loaded library
values decline writes. Projections decide how unrelated fields survive an edit.

Navigation landmarks contain their selection callbacks. Moving onto an
editable line installs the description produced by that projection, including
the intended caret position. The shell does not inspect the render tree to
infer editability. See [navigation](../progred/src/navigate.rs).

## Completion

The projection rendering a pending value or label explicitly requests
completion and may supply a lazy vocabulary. Only the active picker asks the
provider for offers. Without one, the editor uses its universal offers.
Root templates and root field vocabulary are supplied separately and do not
leak into descendants.

The placed frame retains the exact visible offers. Each offer has an activation
callback plus explicit text, styling, matching spans, and source attribution.
The reusable Puri widget draws rows; Progred owns the document operations,
query state, scrolling, and floating card. See [offer construction](../progred/src/completion.rs)
and [completion presentation](../progred/src/projection/completion.rs).

A provider starts with its narrow vocabulary. The trailing `…` participates in
row navigation but activates expansion rather than committing a value. Expansion
keeps the selected index, clamped to the available rows. Changing the query
resets selection, scroll, and expansion; returning to an older query does not
restore an old choice. Pointer and keyboard activation use the same callbacks.

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
state. Surviving declarations retain their view state. Moving an entry assigns
a new list position and explicitly retargets selection; its size, scroll, and
projection mode carry across, while path-keyed folds reset.

`{value: source, projection: function}` is a presentation-library convention,
not workspace configuration. It can appear anywhere a value is projected;
see [projection composition](projections.md#projection-composition).

## History, gestures, and persistence

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
