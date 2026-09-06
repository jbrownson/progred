# GID

GID is Progred's native data substrate. It is both the logical model the
system manipulates and the future binary storage stack for that model. It
is not a syntax tree, a textual language, or a binary encoding compiled
from text.

The logical model currently consists of:

- opaque 128-bit cell identities;
- values made from cell references, blobs, lists, and records;
- a direct cell-identity-to-value table, where an absent entry is a bare cell;
- documents containing one optional root value and their cell table;
- stable list positions and traversal steps used while manipulating a document;
- stable resolution sources: a `Follow` step names the document or library
  whose value it crosses. Duplicate definitions across sources are tolerated,
  but ordinary lookup selects one; they are not implicit composition.

Names, UTF-8 text, numbers, Grap, and CAD concepts are open
conventions or libraries embedded in GID values. They are not primitive
GID forms.

The native binary representation is not designed yet. It should follow
the model directly and eventually support the needs of a projectional
system: direct manipulation, cross-document references, indexing, and
partial or lazy access where useful. Those goals must not be constrained
by the temporary text bridge.

The path library represents traversal steps and list-position bytes as ordinary
records, lists, and blobs. This is a library convention, not another GID atom.
Runtime list positions still have session lifetime; serializing a path as data
does not make its element addresses survive a document reload.

## Text bridge

[`gid-text.md`](gid-text.md) documents the current binder-oriented text
import/export notation. It exists for hand authoring, Git, debugging,
LLM tooling, and other text-bound systems while Progred's own authoring
matures. Its binders, parser leniencies, and textual layout are not GID
semantics. Checked-in `*.gid` files are fixtures for that bridge;
`.gid` is reserved for the future native representation.
