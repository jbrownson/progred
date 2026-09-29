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
- stable list positions and traversal steps used while manipulating a document;
- stable resolution sources: a `Follow` step names the document or library
  whose value it crosses. Duplicate definitions across sources are tolerated,
  but ordinary lookup selects one; they are not implicit composition.

Names, UTF-8 text, numbers, Grap (including its tagged absence values), and
CAD concepts are open conventions or libraries embedded in GID values. They
are not primitive GID forms. UTF-8 text, for example, is the open
`libraries::text` record convention over a blob.

A `CellId` is not an RFC 4122 UUID: all 128 bits are random, with no version
or variant fields. Every textual spelling, including `Display` and
human-readable Serde, is 32 lowercase hex digits with no hyphenated form;
binary Serde uses the 16 bytes. Well-known library cell IDs are once-minted
random identities, never names or hashes of names; their readable names are
ordinary `libraries::name` facts. Mint a fixed identity from 16 unmodified
OS-CSPRNG bytes (`new_cell_id`, or `openssl rand -hex 16` for a source
literal), never from a UUID generator.

The Rust implementation shares record, list, and blob storage across clones.
Blobs wrap `Arc<Vec<u8>>`: construction takes ownership of the existing byte
buffer without copying it, and `make_mut` copies shared bytes only when edited.
There is no size cutoff, interning, or storage identity in value equality,
hashing, or serialization. See [the storage measurements](performance.md).

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
`.gid` is reserved for the future native representation. When that lands, it
takes over the extension and the bridge moves aside.
