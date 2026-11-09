# GID

GID is Progred's native data substrate. It is both the logical model the
system manipulates and the future binary storage stack for that model. It
is not a syntax tree, a textual language, or a binary encoding compiled
from text.

The logical model currently consists of:

- opaque 128-bit cell identities;
- values made from cell references, blobs, lists, and records. Cell
  references and blobs are the only atoms, and record labels are always cell
  identities;
- a direct cell-identity-to-value table, where an absent entry is a bare cell;
- documents containing one optional root value and their cell table;
- stable list positions and traversal steps used while manipulating a document.
  A `Follow` step crosses a cell reference into the cell's definition, wherever
  lookup finds it; which tables lookup searches, and in what order, is up to
  the caller.

Names, UTF-8 text, numbers, Grap (including its tagged absence values), and
CAD concepts are open conventions or libraries embedded in GID values. They
are not primitive GID forms. UTF-8 text, for example, is an open record
convention over a blob. Grap keeps its encoding beside f64's, in `grap::text`,
and the text library owns its names, functions and projections.

A `CellId` is not an RFC 4122 UUID: all 128 bits are random, with no version
or variant fields. Every textual spelling, including `Display`, is 32
lowercase hex digits with no hyphenated form. Well-known library cell IDs
are once-minted random identities, never names or hashes of names; their
readable names are ordinary `libraries::name` facts. Mint a fixed identity
from 16 unmodified OS-CSPRNG bytes (`new_cell_id`, or `openssl rand -hex 16`
for a source literal), never from a UUID generator.

Only the atoms have text forms of their own, and each type that has one writes
it with `Display` and reads it back with a `read` returning `Option`: a cell
id is 32 hex digits, a blob is `0x` and hex digit pairs. Records, lists, and
values have no text form in GID; the text notation spells them.

The Rust implementation shares record, list, and blob storage across clones.
Blobs wrap `Arc<Vec<u8>>`: construction takes ownership of the existing byte
buffer without copying it, and clones share it.
There is no size cutoff, interning, or storage identity in value equality,
hashing, or serialization. See the blob microbenchmark in
[performance checks](performance.md).

The native binary representation is not designed yet. It should follow
the model directly and eventually support the needs of a projectional
system: direct manipulation, cross-document references, indexing, and
partial or lazy access where useful. Those goals must not be constrained
by the temporary text bridge. Unlike text, which people write and which may
list a record's labels in any order, it should write labels in increasing
order and reject anything else as corrupt.

The path convention represents traversal steps and list-position bytes as ordinary
records, lists, and blobs: a path is a list of `{key: cell}`,
`{element: {indexable: blob}}`, and `{follow: {}}` steps, and decoding checks
each step and the canonical list-position bytes. A follow step saved with a
source, as earlier paths were, reads as a plain follow. This is a library
convention, not another GID atom.
List positions are saved with the document, so a path serialized as data,
element addresses included, survives a reload.

## Text bridge

[`gid-text.md`](gid-text.md) documents the current binder-oriented text
import/export notation. It exists for hand authoring, Git, debugging,
LLM tooling, and other text-bound systems while Progred's own authoring
matures. Its binders, parser leniencies, and textual layout are not GID
semantics. Checked-in `*.gid` files are fixtures for that bridge;
[deferred work](deferred.md#native-gid-storage) reserves the extension for the
native representation.
