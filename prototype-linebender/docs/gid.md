# Gid

GID is the format's name; an identifier is a GID ID (settled
2026-07-23 — the extension `.gid` has no living claimant, only
WinHelp's dead index files and a niche simulation tool's project
directories). This document specifies the textual form: the raw
projection's grammar, made writable. It exists to bootstrap — hand- and
LLM-authoring of documents and libraries while the editor's own
authoring matures — and a binary sibling will join it when authoring
moves inside the editor (same entries, no binders; a sibling, not a
successor). No version field, migration branch, or compatibility reader
until files exist beyond this repository; model changes update the
parser and checked-in files together.

## Doctrine: saving canonicalizes

The cells serializer's rule was "parsable means canonical." This
format deliberately relaxes it: the PARSER is lenient in the defined
ways below — nowhere else — and the PRINTER is the canon. Loading a
lenient file and saving rewrites it canonically. Strictness moves
from the reader to the writer, because being writable by hand is the
format's purpose. Structural damage is still an error: an unbalanced
brace or quote, a duplicated label within one record, a blob of odd
length, an unknown escape, or a cell label with no following value.

Defined leniencies, exhaustively:

- gid case (canonical output is lowercase),
- trailing commas present or absent,
- whitespace anywhere between tokens,
- binders used without a `binders` entry: a gid is minted on load
  and the binder persists on save,
- any of `binders`, `cells`, `root` absent (an absent root is the
  empty document).

Stating the same cell twice — by any spelling: two same binders, or
a binder and its raw gid — is an ERROR, not a leniency: failing
beats clobbering, and it matches the value grammar's duplicate-label
rule.

## Lexical

- **gid** — exactly 32 hex characters, bare (no quotes, no hyphens).
  Case-insensitive on read, lowercase on write. Anything violating
  length or alphabet is not a gid. (Gids are 16 CSPRNG bytes — not
  RFC 4122 UUIDs — and the notation's spelling reflects that.)
- **binder** — a bare token `[A-Za-z_][A-Za-z0-9_-]*` that does not
  parse as a gid. Binders are FILE-LOCAL names for gids: pure
  serialization sugar, never part of the loaded model, invisible in
  the document.
- **The two-namespace rule** — a bare token anywhere an identity can
  stand is a gid if it parses as one, else a binder.
- **quoted text** — double-quoted, escapes `\"` `\\` `\n` `\t`.
  In value position this is surface sugar for the `progred-text`
  `{utf8: <blob>}` convention, not a core atom.
- **blob** — `0x` followed by an even number of hex digits;
  lowercase on write.
- Punctuation: `{ } [ ] : ,`.

## Values

The value grammar is the raw projection's:

- `{label: value, …}` — a record. A label is always a cell identity,
  written as a bare gid or binder (the label IS a link). Quoted labels
  are invalid. Duplicate cell identities in one record are an error.
- `[value, …]` — a list. Element positions are session identity,
  minted at load, stripped at save, never written.
- Quoted text convention values and blobs as above.
- A bare token in value position is a reference — a link to that
  gid's cell.

## The file

A file is one reserved envelope with up to three fields. Its quoted
keys are file syntax, not ordinary value labels:

```
{
  "binders": {
    "color": 4c945c6c52b304eb0c2d1503de6d8f77,
    "name": 02e562654d6d0828d3a7559e6f75fffe,
    "payload": e8160795427c912458edc7e28d75a8cc,
    "shape": 777d80d6e03e9ae0f9c143678ab68a75,
    "stroke": 34e8ba540a0297748f92540743779d3f,
    "style": e64688dc84e4b835d86d6c1f4ad5726f,
    "swatch": 0f3ae682742540de963d02d5f4b1a5a5,
  },
  "cells": {
    name: {name: "name"},
    roof1: {name: "roof", stroke: "hairline"},
    21b4fa5c9d2c40de963d02d5f4b1a5a5: {name: "roof"},
    9d2c1e10ab3440de963d02d5f4b1a5a5: {payload: 0x663399},
  },
  "root": {shape: roof1, style: 9d2c1e10ab3440de963d02d5f4b1a5a5, color: swatch},
}
```

- **`binders`** — binder → gid. Keys are quoted strings but must fit
  the binder token grammar (or the file errors), since binders stand
  bare at use sites. `binders` must PRECEDE any use of them
  (resolution mints as it parses, so a late table would collide with
  its own mints); canonical output always puts it first. The table
  earns its keep for gids DEFINED elsewhere — a library cell a value
  references — and for spelling this file's own cells readably. In a
  binary format it would have no reason to exist.
- **`cells`** — a RECORD directly from identity labels to values: the
  gid is the unique key, so the structure says so, and the labels are
  literally cell labels (the file is a value). An identity label is
  a bare token under the two-namespace rule — a gid literal, or a
  binder (bound in `binders` or minted on first use). There is no
  entry wrapper and no metadata half: `{name: "roof"}` above is the
  cell's actual record value. Stating the same CELL twice is the error
  above; equal values and duplicate name facts are ordinary data.
- **`root`** — the document's root value.

The `name` in the example is not GID syntax. It is a binder for the
well-known `progred-name` cell, used as an ordinary record label. The
bootstrap printer recognizes a text-convention value there as one optional
hint for readable binders and ordering. The fact remains graph data:
it need not exist or be unique, and other languages may use richer or
entirely different naming conventions. Binders are file-local
serialization sugar and must be unique.

## Load

1. Read `binders` into the binder table.
2. Walk `cells`: resolve each identity label (gid literal | binder,
   minting unbound binders); the same cell stated twice, by any
   spelling, is an error. Store the following value directly in the
   cell table.
3. Read `root`, resolving references through the same binder table,
   minting unbound binders (a reference to a never-defined binder
   yields a bare cell — create-on-reference at the file layer).
4. The binder table survives OUTSIDE the model — store-layer state
   beside the document path — for the printer. Nothing in the loaded
   document knows a binder existed.

## Save

The printer is deterministic from (document, binder table):

- Binders: loaded and minted binders persist while their gids remain
  mentioned; entries for vanished gids drop. Every remaining identity
  gets a binder. A direct simple-name fact supplies the sanitized base;
  otherwise the base is `_` plus the final five gid digits. Collisions
  receive `_2`, `_3`, and so on. This is a bootstrap presentation
  heuristic, not GID semantics.
- `binders` sorted by binder; omitted when empty.
- `cells` entries: identity labels spelled by binder; cells with direct simple-name facts
  first sorted by (name, gid), then the rest by gid. Their values use
  the ordinary value printer; there is no special name-before-value
  layer.
- Records print their labels in the model's canonical label order.
- Block layout with trailing commas and two-space indentation;
  short leaf-only forms may print flat. Layout is structural only —
  never width-dependent.

## Parked

- A raw-structure load/save mode (the file's own record as an
  editable document, gids encoded as strings) — wanted once
  authoring lives inside the editor.
- A gid atom in the data model itself.
- Numeric convenience literals (an extension over tagged blobs).
- The binary sibling format.
- Domain-specific binder-hint dispatch beyond the bootstrap simple-name
  convention.
