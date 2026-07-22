# Data And Editor Model

Date: 2026-07-03

## Data Layer v3: Values and Cells (2026-07-20)

Decided across the lambda-foundations exploration (the
~/git/lambda-editor sessions, 2026-07-14..20; verified research map
in the session artifact; journey record with the dead ends in
experiments/lambda-calculus-exploration.md). Written as a brief so
the next session starts warm; SHIPPED that session, same day — see
the postscript for what landed and the corrections made in
conversation (the brief was an LLM summary of the exploration and
overreached in one place, the number convention).

The arc. The JSON-shaped rejection (below) gave as a reason:
"coupling identity semantics to shape (records mutable with
identity, lists immutable values) is an arbitrary asymmetry" — and
v2 built exactly that coupling with the poles swapped: maps are the
identity-bearing entities, lists are values. The smell it left
("some things have identity and some don't, by shape") is that
paragraph's own indictment. The third design neither v1 nor v2
considered: NO shape has identity — identity is its own construct.
All shapes (records, lists, atoms) are pure structural values
compared by content; identity is a CELL — a minted uuid holding one
current value. The lineage is Clojure/Datomic (values immutable and
structural; an identity is a succession of values; egal equality:
identity-compared mutables, content-compared immutables), and the
frame is unapologetically "what if JSON were a graph": JSON with the
atom set corrected and refs added.

```rust
pub type CellId = Uuid;                          // was NodeId
pub enum Atom  { Cell(CellId), String(String), Blob(Vec<u8>) }
pub enum Label { Cell(CellId), String(String) }  // narrowed from Atom
pub enum Value {
    Atom(Atom),
    List(im::OrdMap<Position, Value>),           // unchanged from v2
    Record(im::HashMap<Label, Value>),           // v2's entity map, moved inside
}
pub struct Cells { data: im::HashMap<CellId, Value> }  // one value per identity
```

Mechanically small — the v2 entity map moves inside the Value enum —
and each thing it buys was previously a wart:

- Kind machinery fully unrepresentable. A cell holds a record today
  and a list tomorrow; "conversion" is set_value. The sticky-kind
  asymmetry (emptied map vanishes, emptied list persists) and the
  no-identity-preserving-conversion-path wart both dissolve — kind
  is a property of the current value, not the identity.
- Inline records. `{x: 1, y: 2}` as a content-compared anonymous
  value — v2 cannot say this; every map pays 16 bytes of identity
  and acquires aliasing semantics whether wanted or not. CAD is full
  of point-shaped data that wants to be a value. Shallow copy/paste
  (2026-07-10, below) becomes expressive rather than limited: inline
  structure travels by value, references alias — the per-gesture
  identity-fate question answered in data.
- Value-vs-reference is the projection signal. An anonymous inline
  definition renders inline BECAUSE it is not a reference; a cell
  reference renders as a name with unfold. (Independently converged
  with the lambda-workbench's declaration/reference model.)
- Identity priced per use. Mint a cell exactly when something must
  refer to it durably — the unifying form of v2's three
  justifications (mutate-while-shared, cycles,
  distinct-despite-equal) is REFERABILITY. Extract-to-cell is an
  authoring gesture (the tree's extract-to-definition). Tooling
  references — selection, diagnostics — are NOT that: they stay
  paths, maintained by the existing rewrite discipline or
  recomputed. Paths are primitive; identity is a durability upgrade;
  a diagnostic never edits the document to point at line 3.

Atom roster. The admission criteria, sharpened: an atom is admitted
when it (a) maps onto the machine world, (b) matters to users, (c)
canNOT be efficiently encoded by the other constructs, and (d) for
the bootstrap set, is required for a human-usable editor that knows
zero conventions. Under these:

- String stays (fails (c) only in principle — encoding text as
  blobs would leave the zero-convention editor unable to show a
  human anything, failing (d)).
- Blob arrives — the anticipated raw-bytes atom, identity = the
  bytes, memcmp equality, a mini hex editor as the projection floor.
  The old codepages rejection is answered, not overruled: codepages
  were bytes masquerading as text; a blob pretends nothing —
  identity stays decidable by strangers, and interpretation is
  projection-level by design. Deliberately an atom, not a
  list-of-bytes: a "byte" value would readmit numbers through the
  service door, and atoms have no interior structure to address.
- Number(f64) LEAVES. Numbers fail (c): the substrate does no
  arithmetic, so a number is a canonical spelling plus an intent
  bit — a projection convention, not an atom. The general convention
  is decimal m·10^e (mantissa + point position; integers are the
  e >= 0 shapes), spelled canonically (10 does not divide m), stored
  as its canonical decimal string. One-spelling-per-value survives
  as convention law owned by the convention, not substrate
  machinery. f64 takes the Blob posture: parked until a real asset
  needs lossless machine-float round-trip (the Fidget boundary
  converts; NaN/-0 canonicalization work shelves with the space).
  Rationals: excluded, library-over-decimals if ever. Reals:
  excluded by this doc's own law — no computable normal form, so
  never a value space; computable reals are computation-layer
  codata.
  (CORRECTED at the landing session, 2026-07-20, user call: the
  decimal-convention half of this bullet overreached — a summary
  artifact, not the decision; the idea had been considered and
  pulled back. Numbers leave with NOTHING replacing them now: no
  decimal convention, no canonical-spelling law, no wrapper shape,
  and the editor's number machinery — the number atom arm, numeric
  query inference, the dual atom offer, the parse-gated number
  write — was REMOVED, not transferred. Number encodings arrive
  later as libraries, plural, over blobs or records as they please,
  none anointed by the data model, and probably not by overloading
  strings. What stands: numbers fail the admission test;
  f64/rationals/reals stay out.)
- Labels narrow to String | CellId. A label MEANS: strings mean
  casually, cells mean by metadata lookup; blobs deliberately don't
  mean, so they can't be labels; number labels die with Number.
  Value-keyed dictionaries are collections, not records — encodable
  as lists of pairs, promotable if a real customer appears.

Migration: format 2; loaders refuse format 1 (house pattern,
prototype scratch); samples re-authored. The number-editing
machinery (parse-gated write-through, dual atom offer, numeric
completion ranking) transfers nearly unchanged as the decimal
CONVENTION's projection — the first atom-level convention layer,
sibling of the list projection. (At landing: the format NUMBER
stayed 1 — no files exist outside this repo to refuse, so only the
grammar changed and old scratch refuses by grammar — and the decimal
transfer did not happen, per the correction above; the machinery was
deleted.)

What re-roots in the editor, the expected cost ledger:

- Write unit: (entity, label) becomes (cell, path). set_value's
  split-at-last-Key generalizes — the last Key step may now address
  a record field at any depth inside a cell's value; respine already
  covers list suffixes; spine_writable becomes cell-of-path with the
  same shape. write_through and History are unchanged in design
  (snapshots of persistent Cells).
- Completion: "fresh node" offers become "fresh cell"; a new inline
  record offer joins (the anonymous `{}`); the label stage narrows
  to Label. Pending machinery otherwise transfers.
- Secondary selection: identity marks are cells and atom
  occurrences; inline records are structure, not identity — no
  marks. (Strings/blobs remain identities like any atom.)
- Graph view: drawn nodes are cells plus shared atom values as
  today; whether inline records render inside their parent or as
  square-cornered value nodes like lists is decided in-session
  (the list precedent: kind is worth a silhouette).
- Sources/library: per-entity fallback becomes per-cell fallback,
  unchanged in spirit; read-only gating keeps the cell as the
  authority unit.
- Floating definitions/orphan pool: unchanged — cells float; the
  keys-as-references note extends to Label::Cell at any depth.

Open questions for the implementing session: root stays
Option<Value> (records may now be the root inline); Record has no
ambient order (raw sorts by label as today — the ordered-edge-set
rejection stands); wrap is now fully subsumed (x -> {k: x} is one
set_value, like lists); naming adopted here — CELL for the identity
construct, RECORD for the value shape, "node" surviving only as the
graph view's colloquial word for whatever it draws.

Shipped 2026-07-20, the session after the brief. What landed matches
the sketch, with these calls made in conversation:

- Records are `im::OrdMap<Label, Value>`, not the sketch's HashMap
  (user, after a first HashMap instinct, on the consistent-ordering
  point): content-compared values want deterministic iteration —
  Eq/Hash/serialization all read it — and label order is the raw
  row order anyway.
- Paths gained a third step: `Step::Follow`, the identity crossing.
  `[]` is the link at the root, `[Follow]` the value its cell holds,
  `[Follow, Key(f)]` a field inside. Forced by cells holding
  non-records: with links followed silently, a cell holding a string
  gives one path two meanings — the link (select, replace, delete
  the reference) and the string (mount the editor) — and the write
  side would need a selection-path-isn't-write-path special case.
  Every write now splits at its LAST Follow: the link before it
  names the owning, authority-gated cell; the Key/Element suffix is
  a pure value spine rebuilt through the new `spine` lens
  (get/set/without in progred_graph) — the brief's "(cell, path)"
  write unit made literal. Each reference site unfolds through its
  own Follow: paths stay per-site, and no site is the value's home
  (the table entry is).
- A cell need not hold a value: BARE = absent from the table, never
  Option (user). Unnamed "new cell" mints a bare id and writes
  nothing; the named mint seeds `{name: …}`, the one convention
  write; "new record" is the separate `{}` value offer — cells and
  records fully separate constructs in authoring (user: don't bias
  what a cell holds). The within chord on a bare cell pends its
  first value at `[…, Follow]`; delete at a trailing Follow removes
  the table entry — bare again, the mint's symmetric partner. In
  data, bare-minted and dangling are one honest state: a uuid in
  link position.
- Blobs: `{"blob": "<lowercase hex>"}` in files (strict reads,
  parsable-means-canonical), `0x` hex as the query and clipboard
  spelling (case-tolerant in — the value is the bytes — canonical
  out), a dim truncated hex form in the tree with no editor. The
  mini hex editor and tagged domain projections (a record pairing a
  reader-identity with the bytes) stay future.
- On screen the HANDLE (name if named, short id otherwise) marks a
  cell and BRACES mark an inline record — two constructs, two
  syntaxes; a `()` cell-delimiter idea was considered and kept in
  reserve if the handle proves too subtle. A cell's record value
  renders as the familiar block under the handle; `{}` after a
  handle distinguishes an empty record from bare; inline records
  read `{x: "1"}` with a braced block form, collapse override-only
  like lists. The record value under a handle stays selectable and
  keyboard-reachable at `[…, Follow]` (no pointer target — clicks
  belong to rows and the cell) — select it to copy or delete the
  value as a whole. (Corrected on first run, below: "handle" here
  misread the user's word — the name text is not cell SYNTAX — and
  the reserve idea landed the same day as the star.)
- The graph view became the IDENTITY graph (user sketched the
  problem — edges from within a cell — and delegated): nodes are
  cells, plus one synthetic node for a non-link root value (a link
  root tints its cell, as before); every link occurrence inside a
  value is an edge, its pill naming the field label, the element's
  1-based ordinal, or a dotted chain for deeper nesting; bare cells
  draw dashed — the red-link look. Edge deletion unlinks exactly
  the field or element holding the link (`spine::without`); cell
  deletion removes the table entry and strips its links everywhere,
  a cell whose whole value was such a link going bare. Values are
  content, shown in the tree; the graph shows who refers to whom.
  Strings and lists no longer draw as nodes (v2's shared-value
  display); whether the boxes should hint at their cell's value
  kind iterates live.
- `Sources` kept its two-sided shape with per-cell fallback —
  cleaner than v2's per-entity, one table entry being the whole
  statement — and the `Gid` trait died with the abstraction (user:
  moving from abstract to concrete).
- The sample: an inline-record root of roles; the roof/points/style
  scene preserved (the cycle, the shared unnamed style, the floating
  `stroke` label cell); the new constructs shown — point positions
  as inline `{row, col}` records, a `swatch` blob, and a bare
  `material` cell referenced before anything is said about it.

Tests: 77 across the workspace; clippy at the pre-rebuild baseline.

First run (2026-07-20, user) pulled three corrections forward. (1)
The STAR is the cell syntax: "handle" had meant a clickable REGION
standing for the cell — like the brackets are the list's — not the
name text, which reads as content. Every cell occurrence now leads
with a dim `*` (the user's suggested spelling), the always-cell
click target — the name text still engages for renaming on the
second click, but the star never means anything but the cell.
Graph boxes speak it too (`*roof`), so the unstarred `{…}` root
value node reads as the value it is, and the dashed outline reads
as what it marks: a BARE cell (the sample's `material`, the red
link). (2) String labels wear their quotes everywhere — tree rows,
inline records, pill chains — answering v2's open styling question
(a quoted string label vs a name-read cell label), and making the
inline record read like the JSON it resembles. A pill like
`"points"·1` is the identity graph's compound: the link at element
1 of the points list, where v2 interposed a list node. (3)
Opposite-direction edges between one pair (roof→corner beside
corner→`of`→roof) arced to the SAME side and overlapped — the
parallel-edge normal flips with edge direction, latent since the
egui era and exposed the moment lists stopped being nodes; offsets
now live in the canonical pair's frame.

NAMES JOINED THE IDENTITY TABLE the same evening (user proposal:
"relatively speaking very ugly, but it makes the editor so nice"),
and the trade turned out better than its billing — three standing
warts dissolved at once. With the name a field of the cell's record
VALUE, conversion destroyed it (set a named cell's record to a list
and the name went with the record, though it named the identity,
which survived); the named mint had to conjure `{name: …}` — a
record forced into existence to carry a name, exactly the
what-a-cell-holds bias the minting design had just rejected; and
the NAME well-known cell, its name-names-itself library entry, and
the consumed-edge machinery existed only to bootstrap the
convention. The exploration's own Unison finding says the quiet
part: names are editor-needs metadata that terms cannot hold,
living in an engineered layer OUTSIDE them — the table IS that
layer. Naming is per-identity, not per-reference (the filesystem
does the opposite: names live in directory entries); a per-reference
URL/URI for locating missing references across documents is parked
as future reference metadata, not foreclosed. The entry is a SUM —
`Cell = Named(String) | Valued(Value) | Both(String, Value)` — after
the user rejected a struct of two Options: a cell with neither is
not distinct from no cell, so the state is unrepresentable and
absence stays the one bare form (a fourth state may earn its way
back when libraries load, with evidence). File form:
`{"name": …, "value": …}`, either half omitted, the empty object
refused. The editor reaches names through a terminal `Step::Name` —
names are not values, so `resolve` never resolves one; instead
Selection::edge, write_through, delete, and writability each grew
one arm (`set_name` beside `set_value`, gating on the NAMED cell's
own authority, not the enclosing owner), which buys selection,
editing, undo, secondary marks, and the two-stage rename click
through the existing machinery. An unnamed cell's short id engages
as an EMPTY name editor on the second click — typing names it — so
keyboard and pointer naming survive the death of the name field
(the old flow, Enter + "name" + value, has no field to complete
into anymore). Named bare cells are now exactly the red link:
create-on-reference mints a name and nothing else, and delete at a
trailing Follow clears the VALUE while the name stays. The name
policy became `Names::table` (still the one swap point for computed
names); the built-in library is EMPTY — the authority machinery
stays, unexercised until real libraries load.

THE PARENS LANDED with it (user: the `()` `[]` `{}` symmetry):
`(` name-or-short-id value `)` is the cell's syntax, the star
retired after one session, and braces returned uniformly — a cell
holding a record reads `(roof {…})`, the closers joining as `})`
and `])` at the block foot; leaf values inline, `(material)` a
named bare cell, `(…4be21 "hello")` an unnamed one. The parens are
plain syntax: clicks on them fall through to the cell's own
descend, which was already the select-the-cell region.

THE GRAPH VIEW simplified to REFERENCE TOPOLOGY (user questioned
whether it still makes sense — "the data structure isn't really a
graph" — and delegated; the call: it keeps earning its demo keep as
the picture of exactly what the tree hides). Nodes are cells (plus
the synthetic root-value node), speaking the paren syntax; an arrow
means "this value mentions that cell," deduplicated, labels counted
as mentions; pills, ordinals, compound spines, and edge selection —
v2 vocabulary with no v3 referent — are deleted, and node
select/detach is the only mutation (which field held a link is the
tree's business). Cycles, sharing, floaters (the stroke cell hangs
unconnected — labels draw no arrows), and dashed red links all read
directly. The someday design if the view earns more: boxes
containing their cell's value as a miniature projection, links as
wires from where they occur.

Second-run corrections (2026-07-20, user), three: (1) THE EMPTY
STRING IS THE CANONICAL SPELLING OF NO NAME — the user asked whether
the name even needs Option ("what does it mean to be named or both
with an empty string?"), and the answer keeps the sum while
normalizing the spelling: `set_name` treats `""` as un-naming, so
`Named("")` and `Both("", …)` are unrepresentable by construction
(the same one-spelling-per-value law the atoms live by), the file
form refuses a spelled empty name (omission is the spelling), and
the editor consequence is pleasant — emptying the name field
un-names LIVE, typing re-names, and the empty editor over an
unnamed cell writes nothing. (2) The empty name editor showed a
collapsed sliver: `text_edit` grew PLACEHOLDER support (the one
Puri extension this needed — ghost text in its own style that sizes
the field while the buffer is empty, the first typed character
snapping the field to fit), and the name editor ghosts the cell's
short id — what an empty name falls back to. (3) A valueless cell
read as a dead end — `(…4be21)` with no way in by pointer. The
Follow slot now renders the pending placeholder (`…`) inside the
parens, and selecting it — click or arrow — begins the first value,
by extending the empty-root rule: selecting an empty value slot is
already authoring it (`Selection::edge` pends on the empty root AND
on a writable valueless cell's Follow). Cmd+Enter still works; the
placeholder makes it discoverable.

Third-run settlements (2026-07-20, from the how-did-it-land review):
RAW SHOWS NAMES — names are identity data now, and Raw shows data;
what stands down in Raw is only the POLICY (computed names,
convention layers to come), so `display_name` is the editor's one
read (raw side the table, normal side the policy) and the Raw
toggle is visually inert today, standing by for the first real
convention layer. No bootstrapping identities remain anywhere —
NAME was the last, and the built-in library is empty. NAMES ARE NOT
EDGES, stated in code: the delete arm at the Name step is gone
(there is nothing to detach — un-naming is emptying the name
editor, "" being no-name's one spelling); the deletion vocabulary
stays for values and fields. The sample gained a `favorite` cell
holding a bare link to the corner — the alias pattern, and the
standing repro for the block-in-row seam (a cell whose value blocks
inside another cell's parens floats the outer paren beside the
block). The empty-slot pend rule got its direct test, external
decline included. Parked from the same review: copy/paste needs a
rethink for name selections (copying a selected name yields
nothing today); cell_view's block scaffolding stays deliberately
un-unified until the rendering is polished enough to know what the
shared shape is.

CLICK TARGETS NARROWED TO CONTENT (2026-07-20, user: deselecting was
unfindable because every hit zone was a bounding box — a block's box
spans its structural whitespace, so most of the window selected
SOMETHING). The rule now: selection highlights, the reveal rect, and
keyboard reach keep the full bounds (`descend` split into the
click-free `descend_landmark` plus explicit content claims), but
POINTER selection requires ink — clicking blank space outside any
content falls through to the background's deselect. What counts as
content, per form: atoms claim their span, quotes included; inline
literals claim their whole line (delimiters, commas, the gaps
between items — the "spaces between words"); field rows keep their
label→value span whole (the gap between is a deliberate target);
block forms claim their header band and their closer line; a cell's
parenthesized head claims the cell everywhere it appears; element
dashes are ink and select their element. What deselects: indent
gutters, inter-row gaps, the dead space right of narrow rows inside
a block's width. The one affordance retired: clicking a block's
interior whitespace no longer selects the block — its head and
delimiters do (user-approved trade). The `Held` frame carries the
cell's target so a held container's delimiters select the CELL
while a standalone container's select the value.

DELIMITERS ARE DRAWN AND STRETCH (2026-07-21, the queued
single/multiline discussion, resolved in one move). The seam on the
table was multiline content in an inline position — the floating
`)` — with two candidate fixes: (a) blockness infects upward, text
style, or (b) delimiters grow to wrap their content, embracing the
non-text medium. The user was inclined to (b); the discussion
sharpened it into something stronger: (b) makes layout
COMPOSITIONAL — every container is a rectangle that wraps whatever
its children turned out to be, no fixpoint, no infection, which is
also what Puri's box-with-baseline model (extents known at
construction) was built for — and it makes the inline-vs-block
choice a typography PREFERENCE instead of a correctness question,
since a "wrong" choice just renders tall. The user then dissolved
the remaining asymmetry: in a rectangle model the textual
dangling-`{` idiom (which buys non-rectangular flow in the DOM) is
not worth rebuilding; a drawn bracket occupies a narrow COLUMN, not
a line, so the block form needs no header or closer line at all.
Delimiters are now drawn vector paths at EVERY height — the
math-font extensible recipe (fixed hooks, stretched waist, constant
stroke), proportions sampled from the system font's own glyphs so
the flat form passes for text (`puri::delim`, tuned in the
delimiter_bench example against skrifa-extracted outlines; stem
0.075 em, paren/bracket ink 0.21 em, brace 0.30 em, glyph span
-0.704..+0.171 em, a stretched delimiter trimming to meet the glyph
span on its first and last lines).

What changed structurally: `Held` IS DELETED — a cell is always `(
head value )` with the parens minted after the content and
stretched over its extent, so cell_view is one shape and the
container views never fuse across a boundary. Records and lists
keep three forms — collapsed count, inline literal (all-leaf, the
arbitrary policy the width pass will replace), and block — but
block is now `row[tall-open, disclosure, rows-column, tall-close]`:
the delimiters span the column, the disclosure sits on the first
line, and both header band and closer line are gone. Claims follow
the ink rule: the delimiters are the container's handles (select,
command-pick); collapsed lines and inline literals stay whole-row
claims. Two phantom-Space bugs retire structurally: inline forms
now check the collapse override first (an override outranks the
layout the content would pick), and a container inside a cell
consults its OWN path's override since nothing overrides it from
above — cell collapse (cycle default, `( head ▸ n fields )`) and
value collapse (`{ ▸ n fields }`) are now two honest layers, each
keyed at its own path. The favorite repro retires with the seam:
closers hug their content at full height, and the overbroad
whole-row claim is gone because only flat forms claim their row.

Rendering is verifiable headlessly now: `cargo test -p progred
svg_bench` renders the sample document through the real projection
into target/raw_projection.svg and a narrower
raw_projection_narrow.svg (glyphs outlined via skrifa) — the
identicon-era qlmanage trick, kept as a test since the crate is a
binary. Graph-view node text still spells `(name)` textually — same
category as popup labels, not part of the delimiter family.

LAYOUT IS A FUNCTION OF WIDTH (2026-07-21, same session — the user
called keeping the all-leaf policy "fiddling around w/ the
incorrect solution"; the width pass landed instead of being
queued). The discipline is greedy outermost-first fit — the Wadler
algorithm, which the user's root-first instinct reinvented; the
fancy global-optimal search (Knuth–Plass territory) was rejected by
both of us as unstable under resize. `project` takes the viewport
width; every container view takes the width remaining at its
position; each decision is one local test. A record or list builds
its literal candidate and keeps it iff it fits the remaining width
AND stayed one line tall (a child that broke inside disqualifies
the literal, however narrow — flat means flat); otherwise the block
form rebuilds the children against its own columns. Build-and-
discard is exactly the side-effect-free-construction invariant
puri's layout was designed around; candidates cost O(depth)
rebuilds and nothing is cached until it hurts. Literal children
build against the parent's full budget rather than a sequentially
decremented one — if a child overflows it, the total does too, so
the parent's own fit test is the gate either way (Wadler's fits
test, verbatim). Field rows are the drift killer: a value that
overflows the room beside its label BREAKS AFTER THE LABEL and
drops to the next line at a fixed 20px tab — never aligned under
the label's width, which is the indentation that accumulates.
Cells never break: head and value share the row, the parens
stretch, and room comes from the field row above them dropping.
Known and accepted: layout can flip literal/block while typing (the
soft-wrap category of motion — revisit with hysteresis only if it
annoys), and when even the block forms overflow a too-narrow
window the content just runs off the right edge (no horizontal
scroll yet). The all-leaf policy and `leaf_atom` are deleted; no
kind-based layout heuristic remains anywhere.

Corrected the same day after first use (user: unusably slow): the
first cut measured candidates by building them with the parent's
budget, so every level built its children twice and the recursion
went EXPONENTIAL in nesting depth. The fix makes the candidate BE
the measurement, built once: literal children get an UNBOUNDED
budget, so every nested fit test passes and the flat form
materializes with no branching inside — the enclosing width test is
the single gate (Wadler's fits test, operationally; the user's
"offer both layouts and let the system choose" model, with the
losing alternative never materialized). Field rows likewise build
their value ONCE against the larger of the two positions' budgets
and read beside-or-drop off the result — a node that fits beside as
a whole cannot overflow there. Construction is now O(n · blocking
depth); the shaping inside is deduplicated by a WITHIN-FRAME memo
(`puri::text::TextCache`, a field on `TextCtx`): caller-owned,
cleared at the top of each pass, keyed by text + full style
identity + scale — the caller-threaded memo table the Puri rules
anticipated, with no invalidation and no cross-frame state. From
the same review: layout answers to the width LEFT OF THE GRAPH
PANEL when it is up, instead of running beneath it; a collapsed
cell is PURE ELISION — no field count — because a cell never
introspects what it holds (user rule; the counting summaries belong
to a container's own collapsed form, where a record describes
itself); and the brace found its identity — waist toward the
terminals so the mid point juts, point vertically tighter than the
hooks — trading a little SF fidelity for paren/brace legibility at
a glance.

Second round of user feedback, same day: CELLS DO NOT COLLAPSE —
"it's always 2 things, we collapse the thing inside the cell" — so
the cell-collapse arm and its disclosure are gone; the one place a
cell still elides is CYCLE RE-ENTRY, where the repeated cell
renders `( … )` (the user's form — a mark of recursion, not a
summary; no head, no triangle) and the collapse override at the
cell's path, via Space on the selection, expands one more turn.
CELLS GAINED A BLOCK MODE (user call, from nested heads eating the
demo's width): the field-row discipline inside the parens — value
built once against the larger budget, beside the head where it
fits, else dropped below at the tab with the parens spanning both.
The TEXT CACHE went CROSS-FRAME by mark-and-sweep (user design):
entries carry a used flag, `sweep` at the top of each pass drops
what the previous pass never touched and resets the marks — keys
carry full identity so a stale entry can never be wrong, only
unused, and the steady state is the visible text shaped once.

Third round, same day: CURVATURE FOLLOWS THE FULL HEIGHT (user: a
sharper mid-point was the wrong axis — at tall sizes both shapes
were mostly a straight vertical line, distinguishable only at the
very center). The fixed-hook-plus-straight-waist recipe is gone
from the curved delimiters: the paren is ONE half-ellipse spanning
the whole height, the brace TWO mirrored S-waves — four
quarter-ellipses, each a quarter of the height, meeting at the mid
point — so a tall delimiter reads by silhouette along its entire
span, and the one-line form is the same shape at glyph height (the
1x brace reverted to its natural proportions by construction; the
extreme waist/point round is superseded). `DelimStyle.hook` is
deleted. TALL DELIMITERS GROW WIDER (user: same-width tall braces
are "super squished") — the math-font rule, TeX's \big through
\Bigg: ink width ramps with the square root of height in lines,
capped at 2x, one-line forms exactly the base widths. Growth costs
layout NOTHING: the first cut reserved the cap in the width budgets
and the over-reservation visibly deepened narrow layouts, so the
model is typographic OVERHANG instead — a delimiter's advance is
always its flat width, the terminals stay where the flat form's
would be, and the grown bow bulges OUTWARD past the advance, the
way a glyph's ink may exceed its advance. Nested delimiters compose
because each bulges at mid-height into the empty side of its
neighbor's column. The brace's halves also split unevenly again
(hook two thirds, point one third) so the mid kink turns twice as
fast as the ends — sharpness WITH full curvature, now that the
straight waist isn't the frame it reads against. Color-per-kind
stays in reserve if shape alone proves insufficient in use.

STROKE CONTRAST (user: "doesn't look like math — any other
inspiration?"; the honest answer was that math delimiters are never
monoline). Delimiters are now FILLED OUTLINES with modulated
weight, the way math fonts actually draw them: thick at the bellies,
thin at terminals and the brace's point, blunt-cut ends instead of
round caps. Construction: boundary curves are radius-adjusted
ellipse arcs — the paren a crescent (outer and inner half-ellipses
meeting at thin vertical caps), the brace two S-wave bands built
the same way per half and overlapped at a blunt point face, the
bracket a thick upright with thin arms. Contrast ramps with height
like width does (near-monoline at one line, so the flat forms keep
their SF match) — TeX display delimiters gain weight the same way —
but the cap is a deliberately modest 1.6x the stem: a display-grade
2.5x read HEAVY against the near-monoline UI face (user), and the
mathy quality comes from the modulation being present, not from
absolute weight. `stem` remains the one base
weight everything derives from. Still available from the same
tradition, unadopted: optical overshoot (curves slightly exceeding
flat bounds) and axis-centering (N/A — ours align to content
spans). The beside-vs-drop choices in field rows and cells now
apply the general rule MEASURED rather than argued (user: "maybe
your version of the check was more honest, the perf difference
seems negligible"): both wrapping widths are computed from the one
built content node, and the narrower violator wins when neither
fits. The one alternative still never tried: re-breaking a value
at the narrower beside budget specifically to keep it beside (the
analog of prettier's hug style) — a second construction, noted,
not built. OVERFLOW PICKS THE NARROWER FORM: a small
container's block form can be WIDER than its literal (disclosure,
dash, and arrow overhead — the user asked exactly this), so when
neither form fits the width, the container keeps whichever is
narrower instead of always breaking; both are in hand at that
point, so the comparison is free. The user then stated the general
rule this instantiates, now canonical: alternatives are tried in
priority order, the FIRST THAT FITS wins; when none fits, the
NARROWEST ATTEMPTED wins, priority breaking ties. (An alternative's
width is only known by building it — measurement IS construction —
so the rule selects among built forms rather than pruning builds.) HORIZONTAL SCROLL landed in the
vertical gesture's exact flavor (2026-07-21): both axes ride the
same trackpad/wheel event through `scroll_document`, clamped
against per-frame maxima (the horizontal maximum is the body width
over the layout width, so it only exists where even the block
forms overflow), same resize semantics (stored offsets may exceed
the max; placement clamps, scrolling collapses to reality), and
the graph panel still wins scrolls inside its own bounds.
Directionality verified on macOS wheel and trackpad (user);
Windows may want the horizontal axis inverted — deferred. Scroll
STATE stays app-side by Puri's own rules (state custody is never
Puri's); the puri-shaped piece — a pure clipped-viewport combinator
and the bar drawing — extracts when SCROLL BARS land, which remain
queued, not a priority (user).

THE HUG, TRIED (2026-07-21, user: "I'd be open to trying it", and
their lisp-style preference — closers at line ends, not new lines —
is the same instinct). Field rows and cell values now have THREE
alternatives in the general rule's priority order: HUG — the value
is built with the room beside its label and breaks inside it as
needed, so a record can open right after the label and cascade —
then DROP below at the tab with the drop position's wider budget,
then the narrower attempt when neither fits. Hug outranks drop
(prettier's priority for its hug style); the cost is a second
build only when hugging fails. The look shifted lisp-ward at every
width: chains hug rightward, closers pile at line ends, the wide
render got denser and the narrow render deeper — values now break
WITHIN their beside column rather than dropping for more room.
DENSITY is a flagged, DEFERRED question (user: "the current layout
looks very dense, I think most people won't like it") — the lever
when taken up is heuristics that preemptively choose block forms
before width forces them. The user kept the hug after use: it
reduces the number of large layout jumps under window shrinking.

The hug's first cut went EXPONENTIAL AT NARROW WIDTHS (user hit it
and diagnosed it exactly: "the tighter the width the more it has
to fall back, and if we have 3 options that goes exponential") —
building both positions meant probes containing probes, felt only
when hugging fails at every level. The fix is the DUAL of the
unbounded-budget candidates: the hug decision probes the value's
NARROWEST form with a ZERO-budget build — every nested fit test
fails, so nothing branches inside and the probe is one closed
construction — and since greedy only flattens where it fits, a
value whose narrowest form fits beside still fits there when built
with the room. One real build follows at the chosen position.
Guards keep the fixpoints closed: no probe when there is no room
beside (the decision is forced), and no literal candidates at zero
budget (they can never be accepted). raw_projection_tight.svg
(320px) joined the svg bench as the permanent canary for the
deep-fallback regime; all three renders complete together in
about a second.

SCROLLING SPLIT ALONG PURI'S OWN SEAM (user: "if we don't already
have a Scroll widget in Puri pull that out now, just don't bother
w/ the scroll bars"): `puri::scroll` owns the pure geometry —
`max_offset` (the per-axis clamp from extents) and
`place_scrolled` (the child shifted by the caller's offsets inside
a clipped viewport) — while offset CUSTODY stays app state, the
LineEditState pattern. The document body now places through it,
with real clipping for the first time. It is a placement ENTRY,
not a composable node, deliberately: wrapping a child inside a
'static leaf closure cannot capture Node<P> for lifetime-carrying
P, so nesting scroll areas needs a first-class clip node kind in
layout — that lands with SCROLL BARS, still deferred. The GRAPH
pane already uses the same clip idiom inline (a leaf that clips
and draws pre-lowered data — which is exactly why it composes
where a node-wrapping scroll area cannot), but its viewport is a
CAMERA (pan plus zoom-toward-cursor, unclamped), not a clamped
scroll; whether both should ride one puri viewport primitive is
QUEUED FOR A DEEPER EVALUATION with the clip-node/scroll-bars
round (user: not digging in now). Candidates
for the same treatment later, toward the typical-widget-library-
with-combinators Puri wants to be (user): the completion popup's
card/list chrome (once completion is nailed down further) and the
scroll bar itself; the disclosure triangle was considered and
DEFERRED as fairly progred-specific (user) — both extraction
candidates parked, noted here. The svg bench now TIMES each
projection, numbers only (user call: no assert — read them when it
runs; single-digit milliseconds is healthy, and an exponential
shows as orders of magnitude): a projection is a per-keystroke
cost and narrow widths are where accidental exponentials surfaced
twice.

Post-audit settlements (2026-07-21, user): EMPTY CONTAINERS have
one form — `{}` and `[]` take the literal whatever the width says;
a block of zero rows is not a representation (an active label
query counts as content and layouts normally). This also removes
the probe's overstated width for empty containers. ROBUSTNESS
POSTURE is explicit: layout is "enough to move forward", corner
cases and a fuzzer are deliberately NOT being chased while the
design is still moving — revisit when it settles. The
field-row/cell hug-drop DUPLICATION stays inline on purpose
(divergence likely while this area churns). `puri::scroll::
max_offset` stays as the scroll-bar round's API surface despite
progred keeping its own clamp formulas — the document pane's
clamp answers to the LAYOUT width while its clip answers to the
WINDOW width, a two-viewport subtlety the eventual widget must
carry. REVEAL-ON-SELECTION now chases BOTH axes, horizontally
against the width the graph panel leaves visible. THE COLON REPLACED THE ARROW (user: the
inline/block syntax split read inconsistent, and "now that it's not
really a graph maybe ':' makes more sense for both"): field rows
and the pending-edge query spell `label: value` everywhere; the
drawn arrow is deleted. THE PENDING EDGE RIDES THE LITERAL: a new
field's label query renders inline in a record literal like any
fragment — authoring alone no longer forces the block form, which
only appears when the literal genuinely stops fitting.

THE PLACEHOLDER IS A BOX (2026-07-21, the empty-document editing
round's first nit: "empty document" was prose and the bare cell's
slot was `…` — the elision mark doing double duty). The
standardized notation for an EMPTY SLOT awaiting a value is a small
rounded-rect outline — a shape from outside the projectional
syntax, the user's carry-over from earlier prototypes — and it is
`highlight_rect` in the dim brush (user: the slot and a string
editor's box "should be the same"). Its charge is the empty line
SHAPED AT RUNTIME (the same parley metrics the engaged editor's
frame takes — no measured constants, one source; user caught the
constants version: "shouldn't we just be computing that value at
runtime?"), which also keeps the charge under the one-line
threshold so a container holding a bare slot can still go flat.
It renders exactly where
authoring can begin by selection: the empty document's root and a
WRITABLE bare cell's Follow slot, where the empty-slot rule in
`Selection::edge` makes clicking the box pend in place (the box
lights into the query editor — the transition is the selection
system's own). An EXTERNAL bare cell renders head-only, `( name )`:
by the affordance-lie rule, a slot that cannot pend must not
invite — a library sentinel is complete, not holed. `…` now means
elision only (cycle re-entry's `( … )`). Everything else still
shows no placeholder until a pending exists — insertion points are
selections, not standing holes. The svg_bench gained a placeholder
scene (empty root + bare cell) beside the sample renders. Second
nit, same round: the EMPTY QUERY was a sliver around the caret.
Role ghosts ("value"/"label" dim in the field, tried first) were
RETIRED — the user didn't love words standing in for absence — and
the settlement is BLANK WITH A MINIMUM WIDTH: the box stays empty,
and the engaged query's FRAME holds the box's own width
(`slot_width`, single-sourced) as its minimum. The minimum belongs
to the frame, not the text box — puri's `text_edit` stays
content-sized (a blank query is a bare caret) and `query_content`
pads the deficit, framed BEFORE its decorate so the popup anchor,
the caret clicks, and the caller's ring all span the frame.
The chrome converged through three user catches into ONE RULE:
custom slot chrome (tighter than the ring) made the box change at
commit; a charged top inset moved the baseline ("the text jumps up
a bit"; fonts were identical — editor and text both pin
SystemUi/14); charged side insets moved it left. The settlement
(user: "we should just draw them in the same way"): `highlight_rect`
is THE box — content rect plus breathing room, rounded — the
selection ring paints it blue, the cold placeholder strokes it
dim, and every box is UNCHARGED overhang over exactly the text
frame. The slot's brief custom-chrome era (slot_rect, slot_ink,
slot_insets, descend's highlight opt-out) is deleted; the engaged
pending is back to the plain generic ring, now over a frame that
matches. THE COMMIT INVARIANT, verified by the bench's transition
pair (`"asdf"` typed vs committed-selected): every glyph AND the
ring path itself are byte-identical across the commit — the whole
slot → pending → value transition redraws one shape and only
changes its paint. What still changes is honest: quotes appear if
you didn't type them, and a string narrower than the frame minimum
tightens. The cold box touches neighbors exactly where the ring
would — the box previews the ring's footprint; tuning that
crowding is `highlight_rect`'s one knob. TUNED same day (user: the
ring-sized box overlapped adjoining brackets and read too
prominent): outset 3 → 2, radius 5 → 4 — sized for the QUIET
wearer, since the cold box stands beside delimiters permanently
where the ring only visits. The whole family moved together
(selection rings included — commit parity requires it), and the
cold hairline now clears a paren's ink.
The CARET itself was parley's leaded line box,
box-filling; `text_edit` now refits it to the native shape — the
line's ascent above the baseline plus half its descent below
(user: "the height above the baseline, maybe a bit more") — for
every editor, not just slots. PARLEY'S EMPTY LINE REPORTS A
PHANTOM ADVANCE (~0.19em; user caught dead air inside an edited
`""`). A zero-width measurement override was tried first, then
REVERTED (user: hand-patching measured values is not the way, and
the structural fix below makes it moot): with quotes as affixes no
code path renders bare empty text where width is observable —
static strings are never-empty quoted runs, the affixed editor
composes nonempty, the empty query sits inside the frame minimum,
the empty name editor is ghost-sized, and the cold box probe reads
only ascent/descent. The phantom stands upstream, unobserved. The
composition itself went (user: "we should be measuring just one
string with two quotes?"): `text_edit` grew AFFIXES — a
prefix/suffix on `LineEditState`, shaped and measured as ONE RUN
with the content but never editable. The affixes are armor: an
edit that bites one declines WHOLE (absorb refuses it — backspace
at content start swallows, on empty content it still declines to
the caller's delete-the-value idiom), the selection clamps to the
span between them, and boundary arrows compare cursors in CONTENT
space so they still decline to outer navigation. String literals
are the first wearer: the quotes are the field's affixes, static
and editing render the same single shaped run (`format!` the
quoted form cold, composed editor hot), and the quotes-select
click vocabulary retired — every click on the literal reports a
caret, a quote click landing at the nearest end. Blob `0x` is the
obvious next wearer. Dispatch behaviors locked by test
(`affixes_are_armor_not_content`), including one PINNED
COARSENESS: leading whitespace in content lets a word-delete's
boundary reach through it into the prefix, and the bite declines
WHOLE — a swallowed no-op where trimming was arguable (parley owns
the range; the decline is the contract).

Popup rounds, same day (2026-07-21): AN EMPTY QUERY BOLDS NOTHING —
the filter's empty needle accepted everything with full-span
matches, and the popup bolded them as if typed; spans now mean
"these characters matched your query", so an empty needle accepts
with NO spans. And "NEW CELL" IS A PLAIN CONSTRUCTOR (user: no
asymmetry) — it was a tail template embedding the typed text as
the name-to-be (`new cell "hi"`, unranked, never bold); now it
sits in the ranked pool beside "new list"/"new record" (and alone
survives the label stage — cells label). The mint is BARE:
`resolve_entry` is pure, no document mutation, naming happens on
the head afterward — the one-step named mint (create-on-
reference's popup vehicle) is retired, and with it the label
stage's mint-records-history special case. THE TYPED TEXT OUTRANKS
BARE IDS (user typed "1" and had to arrow past id references —
"if you want to reference something give it a name"): the
strong-before-atom promotion now requires a NAME. Named cells and
the constructors keep it; an unnamed cell's short-id match,
however exact, ranks after the atom the query spells — ids are
for reading, names are for reaching. Pinned by test: an exact
short-id query still offers the string first.

CELL COLLAPSE, THE STORY STRAIGHTENED (2026-07-21, later — user:
"I think there was some miscommunication"). Cells DO collapse,
generally: the earlier "cells do not collapse" round retired only
the SUMMARY form; the elision `( … )` is every valued cell's
collapsed form, one Space away. The mechanics were never the gap —
the semantics are exactly the user's statement of them: a SPARSE
map of per-path overrides (a flat HashMap, not a trie — no prefix
structure), empty on fresh launch; every render looks its path up
and an absent entry means the default, which is FALSE except a
cell inside a cycle. Layout never enters into it: inline literals
collapse exactly like block forms. What was missing was the way
back open by pointer: the elided cell's ellipsis was a select
target. It now TOGGLES via `toggle_target`. And the COUNT
SUMMARIES ARE GONE (user): collapsed containers no longer say
`{ ▸ n fields }` — every collapsed thing is UNIFORM ELISION,
`( … )` `{ … }` `[ … ]`, delimiters selecting (so a collapsed
thing still copies and deletes), ellipsis expanding; `count_text`
deleted. THE TRIANGLES WENT WITH THEM (user: "the > buttons just
sort of pollute, maybe we get rid of them altogether"): SPACE ON
THE SELECTION IS THE COLLAPSE GESTURE, the one way to close
anything, and the clickable `…` the one way back open by pointer
— `disclosure` deleted whole, block forms tightened by the gutter
it occupied. CYCLES are
the same machinery with the default flipped: the repeated cell
defaults collapsed at each depth's OWN path, so expansion follows
the cycle turn by turn, as deep as wanted. Test-pinned:
turn-by-turn expansion, sparse overrides restoring defaults,
plain cells and containers toggling alike, valueless declining. The frame minimum became `puri::layout::min_width`
— a real combinator (pad-to-minimum on the right), not inline
arithmetic — the name editor's content-persists-chrome-marks-
engagement pattern applied to absence itself. The pending
edge's `label: …` tail became `label: ▢` — the value to come is an
empty slot, and empty slots are boxes. THE BOX IS THE INACTIVE
PENDING in code as well (user's picture): `placeholder` is ONE
WIDGET in the Puri idiom, progred-owned — its single state input
is the engaged pending's `Option<(query, choice)>`, None IS the
inactive pending, and identity chrome (descend, highlight, clicks)
stays with the caller.

THE HUG INVERTED — OUTERMOST BREAKS FIRST (2026-07-22, user: "the
higher level constructs seem to be reluctant to start going down
to the next line putting all the compression on the inner
constructs, that seems backwards to me"). The 07-21 hug probed the
value's NARROWEST form — hug whenever the value could SURVIVE
beside its head in any shape — so a parent surrendered its
preferred position only when the child, fully crushed, still
failed to fit: inner alternatives exhausted before outer ones
moved, every hugging level's head bitten out of every descendant's
budget, density piling rightward. The probe now asks the literal
gate's own question — can the child stay WHOLE beside? — an
UNBOUNDED build (closed: every nested fit passes; it is the
literal candidates' construction) in place of the zero-budget one,
so the first break lands at the outermost level that cannot stay
flat and the drop's wider budget lets leaves keep their literals:
Wadler's ordering at both choice points instead of one. The old
overflow tie-break (narrower total when nothing fits) FELL OUT of
the rewrite: hug's total is narrower exactly when head+gap <= tab,
which is the new guard — a head narrower than the tab hugs
whatever the value does, since dropping there buys no room — and
the guard doubles as the lisp-flavored broken-beside form's
remaining home and as the unbounded fixpoint's closure (beside >=
avail-tab at infinity), while the no-room short-circuit closes the
crushed one: forced budgets still never probe, and field_row's
forced-drop early return is subsumed. Evidence at 560px: before,
`origin`'s record shattered one token per line in the right-edge
ravine while the favorite chain sat flat on line one; after, every
leaf literal survives whole at NEARLY THE SAME HEIGHT (483 -> 492)
— the ravine was wasting the vertical it hoarded. The 900px render
pays real height (290 -> 419) as head-only lines stair-step where
chains used to hug — the accepted trade, and the open question if
the dangling-colon lines grate is a HANGING form: the value's open
delimiter hugging the head line, prettier's object style, which
needs a first-line-offset layout Puri does not have. Bench
timings held single-digit milliseconds at all three widths.

THE DASH RETIRES (2026-07-22, user: "I'm not sure the - is
necessary on the list projection in block mode?"). The block
list's leading dash was YAML's crutch — the element-start marker
for a notation that has no brackets. Raw's block lists have drawn
brackets spanning the column, every multi-line element carries its
own delimiter, and each value's ink already selects its element
path; the dash restated all three, and its select_target
duplicated a selection the value's own view offers. Element rows
are bare values now, flush inside the bracket, and their budget
recovered the dash-and-gap overhead. (The earlier click-claims
note "dashes are ink and select their element" describes the
retired form.)

LABELS RENAME BY RE-OPENING THEIR QUERY (2026-07-22, user:
"editing a label doesn't mean going into a string-edit... it means
re-opening a pending and seeding it w/ the current thing"). The
principle that shapes it: VALUES WRITE THROUGH; ADDRESSES STAGE. A
string value edits per keystroke because every intermediate
spelling is a legal value at the same address; a label is an
address — a key in the record's OrdMap — so write-through would
re-sort the row on every keystroke and a spelling that crossed a
sibling's key would clobber its value. So the label re-opens as
the pending-edge machinery itself: PendingEdge grew `replacing:
Option<Label>`, the query seeded with the current SPELLING — a
string label with its quotes, a cell label by its name (a
same-named other cell may rank first; the seed is a spelling, not
the identity — user-accepted) or short id. An untouched seed's
choice zero resolves back to the same key, and committing a TAKEN
label — that no-op included — NAVIGATES to its field, the
new-field rule reused verbatim: a rename never clobbers a sibling;
replacing one means deleting it first (user: "we simply change
focus to it to communicate that it exists"). A fresh label re-keys
through `rename_field` — one set_value of the record with the key
swapped, the value carried, one history step. Engagement is the
Finder pattern's second act (`rename_target`): cold, the label
shares the head's select claim; on the SELECTED field a plain
click re-opens it (command-click still picks). The query renders
in place (`rename_query`) wearing the primary ring explicitly — a
pending edge has no path of its own for descend to mark — colon
and value staying put; a rename forces its record open and rides
the flat literal like any authoring query; Backspace on the empty
query returns to the field, Escape deselects. Name editors stay
write-through by the same principle's other half: a name is the
cell's own metadata, duplicates legal, no shared namespace to
collide in. Scene raw_label_rename.svg pins the notation.

## Data Layer v2: The Typed Model (2026-07-09; superseded 2026-07-20, see v3 above)

The substrate, whole:

```rust
pub type NodeId = Uuid;            // 16 CSPRNG bytes; the only reference

pub enum Atom {                    // the values that can MEAN — every map
    Node(NodeId),                  // key is one; meaning is looked up
    String(String),                // through it (nodes carry metadata;
    Number(Number),                // strings/numbers are their own spelling)
}
pub struct Number(f64);            // canonical: one NaN bit pattern, -0.0 =
                                   // 0.0; Eq/Hash by bits, Ord by total_cmp

pub enum Value {                   // anything sayable
    Atom(Atom),
    List(im::OrdMap<Position, Value>),
}
pub struct Position(Vec<u8>);      // session-only element identity: a
                                   // canonical binary fraction, minted at
                                   // load/insert, stripped at save, IGNORED
                                   // by Value's hand-written Eq/Hash/Ord;
                                   // deliberately neither Value nor Atom,
                                   // so positions cannot occur in data

pub struct MutGid {                // the entity table: maps are the only
    data: im::HashMap<NodeId,      // entities — identity-bearing, mutable
        im::HashMap<Atom, Value>>, // in place, shareable, cycle-capable
}

pub enum Step { Key(Atom), Element(Position) }   // projection paths

pub struct Document { root: Option<Value>, gid: MutGid }
```

File format (versioned; loaders refuse other versions rather than
guess): `{"format": 1, "root": <value>, "gid": {"<uuid>": [[<atom>,
<value>], …]}}` where `<atom>` is `{"node"|"string"|"number": …}`
(non-finite numbers spell `"nan"`/`"inf"`/`"-inf"` — the general form
that used to catch them is gone) and `<value>` adds `{"list":
[<value>, …]}`, inline and recursive, positions never serialized.

The decision arc (all 2026-07-09, one long session):

- Lists were promoted to entity-kinds in the morning (see Lists) and
  the kind machinery immediately bred special cases — sticky kinds,
  panics, write gates, deserializer refusals, a no-names rule. The
  user named the smell ("so many special cases forming") and the
  ground-up question: do lists need identity at all?
- They don't: identity earns its 16 bytes for mutate-while-shared,
  cycles, and distinct-despite-equal. Maps need all three; sequences
  are owned by their containers in practice, and the model already
  declared list-element identity to be session fiction (positions
  stripped at save). So lists became VALUES — inline, structural,
  compared by content (positions ignored) — and every kind gate
  became unrepresentable instead of policed. Fractional positions
  survive unchanged as the session spelling of element identity; the
  container is an ordered map so the every-keystroke value rewrite is
  a structural share, not a rebuild.
- Labels narrowed to atoms BY TYPE (user: a label MEANS — you look it
  up for metadata; a list collects, it doesn't mean). `Step` gained
  the honest second constructor; a dangling `Element` is the same
  stale-path class the editor already tolerates.
- Atoms closed to a fixed enum (user, going practical): the open
  (space, payload) generality had zero users once the position space
  died, and a closed enum lets types own the invariants — UTF-8 by
  String, canonical floats by one newtype — deleting the payload
  disciplines, strict-read machinery, and the general serialized
  form. Strings-only (numbers parsed out) was rejected: it breaks
  one-spelling-per-value or smuggles the number space back as
  spelling discipline, and erases a distinction the editor already
  surfaces (the dual atom offer). f64 stays; arbitrary-precision
  would be a contained future swap of the Number variant's interior.
  If extensibility returns it is one `Blob`-style variant tagged by a
  minted NodeId (so a library can name and describe the tag), added
  when a real payload needs it.
- The beauty trade, named: the founding `Id → Id → Maybe Id` had the
  beauty of the untyped lambda calculus — one sort, all guarantees
  dynamic — and the week's chipping was each position's real sort
  coming home into the types. The signature was quietly false (values
  never had edges, labels never meaningfully took lists); the chips
  removed falseness, not beauty. What remains is the same
  partial-function heart, typed: `NodeId → Atom ⇀ Value`, with
  `Value = Atom | [Value]` — two mutually recursive lines, every one
  load-bearing. Both poles were coherent; the unstable place was the
  middle (uniform signature plus kind machinery), which is exactly
  where the special cases bred.

Related work — why this isn't just triples (RDF, examined
2026-07-09): RDF is the uniform-relation model field-tested for 25
years, and its history is a sequence of retrofits that each re-add a
distinction the uniformity erased. Its positions were never uniform
(literals can't be subjects, predicates must be IRIs — the sorts were
there from day one). Its lists are its most famous wound: cons cells
(`rdf:first/rest/nil`, ill-formed lists representable, every consumer
polices) and ordinal containers (`rdf:_1, rdf:_2` — the re-addressing
problem), with the ecosystem's real answer being "don't put sequences
in RDF." IRIs entangle identity with naming authority (httpRange-14);
minted-uuid-plus-name-edges is the fix. Blank nodes made anonymity
semantically special (existentials) and poisoned merge, diff, and
canonicalization; minting real ids for everything (skolemization by
construction) dodges the class. Literals are (datatype IRI, lexical
form) = our old (space, payload); practice converged on the fixed
core set, mirroring the fixed-atom decision, and RDF's term-vs-value
equality split (`"1"` vs `"01"^^xsd:integer`) is the permanent bug
source our one-canonical-spelling rule collapses. Named graphs are
provenance retrofitted — `Sources` learned that lesson pre-pain.
Parked decisions taken from the comparison: EDGE METADATA, when
wanted, is an ordinary map describing the (entity, key) pair — no
edge identity creeps into the substrate (RDF reification/RDF-star,
Wikidata qualifiers are that pressure); PASTE has two axes — the
projection's spanning tree is the copy boundary (inline = carried,
reference = kept), and identity fate is per-gesture, paste-as-copy
reminting with one substitution map over the deduplicated entity set
(a diamond stays a diamond) vs paste-as-reference keeping ids (the
library fork), never an edge meaning "these ids are equal"
(owl:sameAs); TYPES, when they come, are closed-world shapes that
CHECK documents (SHACL stance) feeding diagnostics and completion,
never open-world axioms that ENTAIL facts (OWL stance) — shapes
check, code computes, nothing infers; interop with triple-shaped
systems, if ever, is an export projection (the Wikidata pattern),
never the native model.

Shipped 2026-07-09, same session. What landed matches the layout;
notes from the build: `progred_graph` is four small modules (value,
position, mutgid, gid) and the old `id.rs` is gone whole — spaces,
payload disciplines, strict reads, the general form. The editor's
write path became TWO functions: `set_value` (split at the last Key
step, `respine` the element suffix — surviving positions kept, the
final step insert-or-replaces — write the one authority-gated edge)
and `delete_edge` (a Key step detaches; an Element step rebuilds the
list without it via the same `set_value`). `write_through` funnels
through `set_value`, so element edits rebuild their list at the
owning edge — string-editing shape, as designed. Editor gates
reduced to `spine_writable` (the entity at the path's last Key step;
no Key step means the document's own root spine) — `pending_edge`
needs only `as_node` (lists decline structurally), the
empty-node-becomes-list gesture died with the ambiguity, and
`Selection::edge` mounts editors on the same test. `list_shaped`
does not exist; `value_view` matches the Value enum. Completion's
"new list" commits `Value::list([])` — PURE, no entity minted, so
the orphan-on-failed-commit wart died for lists — and the label
stage simply lacks the offer. Graph view: nodes are Values (equal
lists = one node, value semantics displayed honestly), lists draw
square and fan ordinal element edges (see the Graph View section —
the first cut drew the inline literal instead and read as a uuid
blob), and node deletion detaches occurrences INSIDE list values too
(a `without` walk), the root list included. The one intentional main.rs seam:
`commit_label` narrows the resolved entry to an Atom and declines a
list — unreachable by construction, stated in one match. Tests:
87 across the workspace, clippy at the pre-rebuild baseline.

## Data Layer v1 (superseded 2026-07-09, see v2 above)

- IDs are GUID | SID | NID (TypeScript-era spelling; the Rust surface
  says node id, string, number). GUIDs are minted identity for mutable
  nodes; SIDs and NIDs are identity the value itself carries. Edges are
  single-valued per label; labels are IDs.
- Lists are graph structure by convention — position-space labels
  whose identity order is the sequence (2026-07-06; previously cons
  cells) — not a data-layer primitive and not values.
- There are no compound values.
- No special fields in the data layer. The editor layer treats some
  labels specially (name, isa, cons) the way a text editor treats
  syntax specially while ASCII stays dumb.

## Atomic Values

(Largely superseded 2026-07-09 by the typed model: the open
`(space, payload)` design gave way to the closed Atom enum, and the
spaces, payload disciplines, strict reads, and general serialized
form went with it. What SURVIVES, load-bearing as ever: one canonical
spelling per value — NaN collapsed, -0.0 = 0.0, exact UTF-8 — now
owned by constructors of the fixed variants; and the Blob-when-needed
posture, returning someday as one enum variant tagged by a minted
NodeId. The reasoning below is kept as the record of why the atom
set is what it is.)

The atom set (SID, NID) is principled, not arbitrary. An atom earns
native substrate status when: its canonical form is free (value =
identity, reflexive equality, no encoding decisions); nothing points a
graph edge inside it (edited as a value, referenced as a whole — graph
selection stops at the leaf, the text cursor owns the inside); it works
in label position, which requires compound-free value-identity; and it
is universal across target domains. Strings and numbers pass; the set
is JSON's atoms minus the ones a graph makes redundant (bool =
well-known nodes, null = absent edge).

Considered and settled 2026-07-05:

- Going lower (char/bit atoms, strings as cons lists) adds nothing:
  one-character SIDs and 0/1 NIDs already express them, so that
  encoding remains available as a convention experiment with zero
  substrate change — while native-only-chars would destroy the string
  labels the naming gradient depends on.
- Blob-with-isa-driven-decode is rejected: identity would depend on
  interpretation (which encoding of 42?), shattering value-identity and
  reintroducing the canonical-encoding problem that killed CIDs. Native
  SID/NID are canonical encodings chosen once, where enforceable.
- A raw-bytes BLOB atom (identity = content hash, meaning via isa above
  the substrate) is the anticipated future addition for CAD assets —
  bytes are the one domain whose canonical form is free. Parked until a
  real asset needs it.
- No integer/float split: `1` and `1.0` as distinct identities is a
  user trap, and Fidget speaks f64. Integerness is a type-layer
  constraint on NID, like all width/format concerns.
- NID equality is total via canonical payload bytes (NaN collapsed to
  one bit pattern, -0.0 to 0.0, at construction). SID identity is the
  exact UTF-8 sequence; normalizing input to NFC is an editor
  convention, not a substrate rule. Atom equality semantics are the one
  hard-to-change part — they bake into every stored document — which is
  why they are pinned here.
- Atom types as libraries was considered (2026-07-05) and resolved by
  layer. Untagged payloads with context-decided interpretation are
  rejected — that is codepages, and the bootstrap needs values readable
  with zero conventions. But UUID-tagged payloads survive the argument:
  `Value(type_uuid, bytes)` with identity as the pair is
  self-describing (MIME, not codepages), the tag being a UUID needs no
  registry (minted once, like name/isa/cons), and unknown tags degrade
  gracefully (tag short id plus opaque bytes). Today's SID/NID enum is
  declared to BE this design with a closed fast-path tag set: `sid:...`
  and bare-number serializations are canonical spellings of the two
  well-known tags. The first atom beyond string/number arrives as
  `Value(tag, bytes)` generalizing all three — not as a third variant —
  which the spelling rule makes non-breaking. The cost to manage at
  that point: with byte-identity, every encoding quirk is an identity
  quirk, so each tag must pin exactly one canonical encoding per value;
  the substrate enforces this for its two tags, libraries must uphold
  it for theirs. The general statement (2026-07-05): a space's values
  are the quotient of its byte strings by an equivalence, and the space
  must split that quotient computably — normalize to a canonical
  representative at construction — because substrate equality is
  syntactic and must stay decidable by strangers with no shared
  conventions; equivalence-as-relation is interpretation-dependent
  identity again. Strict reads are the section's image: parsable means
  canonical. Corollary: domains with no computable normal form
  (programs up to equivalence, graphs up to isomorphism) can never be
  value spaces — sameness there is a tool's judgment over graph
  structure, not an identity. Presentation and editing of atoms are
  already libraries (projections decide how a NID displays); dates,
  colors, units, vectors remain libraries over the existing atoms
  rather than new tags unless binary payload genuinely pays.
- Executable equality (spaces shipping an eq function, e.g. as WASM)
  was considered and rejected (2026-07-05): the equivalence laws and
  hash-congruence become unverifiable promises, fuel-metered
  termination makes map lookup partial, and the eq blobs themselves
  are values whose sameness is function equivalence — the corollary
  above biting its own tail. What survives is the factored form: ship
  the section, not the relation. A normalizer
  (`n : bytes -> Option<bytes>`, run once at construction, metered,
  failure = unparsable) induces an equality that inherits every law
  from syntactic equality for free, so a future library-defined space
  may carry a normalizer blob through the same executable-convention
  slot as user-defined projections. The built-in spaces already work
  this way; `From<f64>` is the compiled-in normalizer. Identity here
  is deliberately intensional; extensional sameness is a tool's
  judgment, never an identity.
- The fixed point (2026-07-05): an identity is `(space, payload)` where
  the space slot is raw 16 bytes with mint-unique convention — raw, not
  an Id, which is what terminates the regress — and node ids themselves
  are just the payload discipline of one well-known space. Strings and
  numbers are two more. This is the spec AND the representation: the
  new prototype's `progred_graph` stores `Id { space: Uuid, payload:
  Vec<u8> }` directly (fields private so constructors own canonical
  payloads), with the well-known spaces keeping their privileged
  serialized spellings and a general `value` form for the rest.
  Consequences pinned now: minting discipline is a space convention
  (content-addressed or externally-issued identifier spaces become
  library-definable — the substrate takes no position on how identities
  are minted), and edge-bearing is a property of the space, held for
  now by exactly the node space; granting it to an eternal space would
  mean documents assert edges about universal values, a deliberate
  future decision, not a default.
- An id whose payload does not parse in its space is a well-formed
  identity without a value reading: identity (equality, hashing, edges,
  serialization) is total over the bytes; interpretation is partial.
  Reads are strict — parsable means canonical — so every value has
  exactly one spelled identity, and near-miss bytes (a non-canonical
  NaN) render as the strange bytes they are instead of impersonating
  the value. Projections fall through to the opaque space:hex
  rendering; documents never fail to load over it; and when a space's
  convention grows, old readers see unparsable-but-valid identities
  rather than errors. This is the atom-level instance of the
  malformed-graph rule.
- All identities in all spaces exist platonically; the substrate stores
  only edges. "Creating a node" is discovering an unused member of a
  mintable space and attaching its first edge; deletion is detachment
  (which is what the orphan pool was always about). The governing rule
  is: a space says HOW, not WHERE — identity answers which, the space
  answers how to read the bytes, documents answer where edges are
  stored, and none may leak into another. Hence two rejections
  (2026-07-05): documents are not identity spaces (location burned into
  identity is the naming conflation again; counter payloads
  reintroduce a minting authority and break coordination-free minting;
  compact ids are an encoding concern — file formats may alias ids
  through a local table, packfile-style, without touching identity),
  and nodes are not singleton spaces with empty payloads (a space is
  the identity of a shared convention; singleton spaces carry none, and
  the flip of the slot's meaning per case is the tell — the old model
  is already exactly embedded as (NODE_SPACE, node id) with its
  spelling preserved).

## Lists

- FINAL (2026-07-09, evening): lists are VALUES — see Data Layer v2.
  The entity-kind promotion below lasted one day as the middle ground
  between the uniform relation and the typed model; its storage
  insight (files keep honest sequences, positions are session
  spelling) and its gesture amendments survive; its kind machinery
  (sticky kinds, panics, gates, refusals, no-names-by-rule) is
  superseded by unrepresentability. Kept as the record of the arc.
- Promoted to a data-model construct (2026-07-09): an entity is a map
  OR a list, never both — kind per entity, declared at the mint
  (completion's "new list") or by the first write's label space, and
  sticky: an emptied list is still a list, an emptied map is nothing.
  The old rejection of first-class lists had conflated them with the
  cons encoding's re-addressing problems; once position labels solved
  ordered identity, the remaining reasons dissolved — ordered and
  unordered edges are never wanted on one entity, so the mixed case
  the unified encoding paid to allow was exactly the case to forbid.
  Storage is the honest sequence, `{"list": [v1, v2]}` beside
  `{"map": [[label, value], …]}`: positions are stripped at save and
  minted evenly at load (`position::spread`), because paths need
  stability only WITHIN an editing session — the position space is
  the session spelling of order, everything below this entry its
  mechanics. The kind boundary is enforced, not tolerated: the data
  layer panics on a kind-violating write (its writers gate first),
  and the editor's one write (`set_value`) declines at the boundary,
  so a malformed list is not a representable state. Old files are
  refused like any unparsable file rather than migrated — prototype
  scratch. A list holds elements only, so the intra-list name edge is
  gone: a list is named by the edge that references it (or a wrapping
  map), and `name` stays a convention — richer naming (multilingual,
  long/short, non-textual) is library evolution, not substrate.
- The lens that settled it (same discussion): the substrate's
  constructs are finite functions — a map a finite partial function
  `Id ⇀ Id`, a list a finite sequence — and they are structure
  because they contain values; atoms have no interior identity and
  compress to spellings (payload bytes in a space). The rule:
  structure where the parts need identity of their own, spelling
  where they don't. What that rule leaves open is honestly
  arbitrary, and arbitrary-but-simpler is allowed to win — which is
  what admitted lists.
- Node operations on a list, the open seam (2026-07-09, user-named):
  the operations that still make sense all work (delete, collapse,
  beside/within, replace-by-rebuild, graph detach); the ones that
  assumed the mixed entity — naming, arbitrary metadata — now have
  nowhere to land, and their landing place is the WRAP idiom (a map
  that holds the list plus its metadata), manual today. Wrap itself
  was demoted by the typed model (2026-07-10, user-observed): under
  value lists, x → [x] is one set_value — no entities, no positions
  to invent — so wrap is ergonomic sugar that copy/paste mostly
  subsumes (cut, new list, paste in), not a structural gesture. The
  reserved path-rewrite mechanism's forcing customer is MOVES;
  wrap rides along whenever that arrives. (Wrap SELECTIONS never
  existed in code — they were the splice design's edge-gap reading,
  demoted with splice 2026-07-06.) Kind CONVERSION has no
  identity-preserving path, and the stickiness is asymmetric: a map
  emptied of its fields vanishes from the gid and is reborn
  kind-free (delete the fields, then Cmd+Shift+Enter — map→list
  works), while an emptied list is still a list, so list→map means
  minting a fresh node and re-pointing references. Accepted for now:
  conversion-in-place is a rewrite of what the entity IS, and if it
  earns a gesture it should be an explicit one, not an accident of
  emptying. Copy/paste, when it arrives as the fork mechanism, must
  carry kind with the edges.
- Encoded in memory as ordered-identity labels (2026-07-06): a list
  is a node whose element edges are labeled by position-space values,
  and the labels' identity order is the sequence. Insertion mints
  `between(prev, next)` and sets one edge; removal deletes one edge;
  no other element's address moves, so the silent re-addressing class
  (delete the first item and a collapse override on the
  second-to-last now collapses the last) does not exist for lists.
  Single-valued-per-label gives one element per position, and `[2, 2]`
  is two edges under distinct position labels — the position identity
  cons cells existed to manufacture, carried directly by the label.
- The position space: payloads are byte strings read as the binary
  fraction `0.b₁b₂…`, so trailing zero bits are value-neutral and the
  canonical form is nonempty with a nonzero final byte — one spelling
  per position, strict reads for free, no headers or length fields.
  Plain lexicographic payload comparison is the dense order, so the
  derived id ordering already sorts lists and the raw projection
  displays them in order unmodified. `between(a, b)` always exists
  and generates bit-aware, so identifiers grow about a bit per
  adversarial same-gap insert — the immutable-label side of the
  order-maintenance trade (Dietz–Sleator; Treedoc, Logoot, and LSEQ
  are the CRDT ancestors). Compaction, if a pathological editing
  pattern ever wants it, is an explicit structural edit through the
  path rewrite, never a silent tax on insertion.
- Supersedes the cons encoding, an FP-background default: tail chains
  address by route, so every sibling edit re-addressed the suffix and
  every edit was cell surgery. Old cons documents remain loadable —
  the raw projection renders any graph — but the convention and its
  well-known ids (head, tail, empty) are retired.
- Ordered edge-sets in the data layer were the third option,
  rejected: ambient order would force meaning onto every record (is
  `name`-first significant?), a freedom nothing wants. Order stays
  opt-in, carried by the labels a convention chooses — each element
  in a stable bucket, the buckets ordered.
- Costs, accepted: no structural tail-sharing (reference the node to
  share), range operations relabel O(n), and traversal is a sort the
  projection already does. Position-identifier schemes interleave
  oddly under concurrent merging; irrelevant single-user, remember it
  if collaboration arrives. (An earlier cost — an empty list needs a
  convention marker — died with promotion: kind is data, so an empty
  list simply exists, `{"list": []}`, the state no shape could say.)
- Superseded (2026-07-09): the empty-list marker was an `isa → List`
  tag on an ordinary node (2026-07-06, the anticipated isa
  convention's first customer); its ids were minted 2026-07-08, wired
  for one uncommitted day, and retired unadopted when lists joined
  the data model. Shape inference went with it — `list_shaped` is a
  kind lookup now, list-ness intentional rather than guessed. Still
  standing from that round: a reserved swap-in empty-list IDENTITY
  stays rejected, because lists are nodes and nodes are shared, so
  swapping the value at one referencing edge silently diverges every
  other reference.
- Moving an element mints a fresh position, so bucket identity does
  not travel with it structurally the way a rewired cons cell could
  have. The answer is the path-rewrite mechanism, not structure: a
  move is an editor gesture, the gesture knows old and new labels,
  and it emits the rewrite that carries collapse, selection, and
  friends across (2026-07-06). A uuid-bucket-plus-ordering-list
  hybrid was rejected as parallel state with an invariant to maintain
  on every edit. Anything a rewrite misses fails loud — state lost,
  never reassigned. Also rejected: a compound label (uuid for path
  identity, fraction for order) — identity by equivalence class is
  exactly the non-syntactic equality the atom rules forbid, storage
  keyed by full ids would relabel on move anyway, and explicit moves
  are already the rewrite's job.

## Rejected: JSON-Shaped Value Model

Considered and rejected 2026-07-03: records-with-GUIDs plus immutable
values (scalars, refs, lists), keys drawn from the value domain. The
reasoning is kept because the idea will recur.

- Value lists force integer addressing. Removing an element silently
  re-addresses everything after it, and this editor retains more
  addresses than most — focus, selection, and splices are explicit app
  data by design, so addresses must survive edits. React's index-key
  fragility is the precedent; sequence CRDTs (RGA and family) mint
  per-element identities for the same reason, presenting JSON lists
  while keeping identity-bearing positions underneath.
- Positions need identity precisely because leaves may not have it: in
  `[2, 2]` both elements are the same NID; only the position
  distinguishes them. Cons cells manufactured that identity — their
  justification, not Lisp nostalgia — and position labels now carry
  it directly (2026-07-06).
- Coupling identity semantics to shape (records mutable with identity,
  lists immutable values) is an arbitrary asymmetry. One law instead:
  GUIDs are minted identity, scalars carry their own, and sequence
  never lives in mutable side-data (an earlier wording, "order is
  structure", overclaimed: ordered identity satisfies the law too).
- Content-addressed compound IDs (Unison-style hashing) were also
  considered and rejected: hashing a graph requires declaring which
  edges are content and which are incidental, and arbitrary graphs have
  no natural content boundary. To share a structure, reference its
  node — that is what the model is for.
- Revisited 2026-07-06 with lists-as-values sketched in earnest: the
  path-addressing argument turned out not to discriminate — label
  paths through cons tails are exactly as positional as integer keys —
  but the rejection stands on the grounds that remain: splice
  selection solves the list projection problem without growing the
  data model, the bootstrapping relief is minor, and the smaller
  substrate wins by default.

What survives is projection-level: the default projection renders list
structure as `[a, b, c]`, and the pitch to programmers stays "like
JSON, except things have identity and can reference each other."
JSON-ness is a view, not the substrate.

## Naming Versus Identity

The core principle is separation of naming from identity, not strong
identity for its own sake.

- SID edge labels are permitted. A SID label fuses name and identity
  for that edge — acceptable for casual data, wrong for schema
  citizens, and the difference should be visible (id- or name-labeled
  GUID keys versus plain-text SID keys).
- No per-key "strengthen" gesture; it would never be obvious when to
  use it. The gradient is contextual instead: records with an `isa` get
  GUID field labels through schema-driven autocomplete; scratch records
  use SID labels. You fall into separated identity by typing the
  record, never by deciding per key.
- Namespaced labels, if ever needed, are GUID labels with path metadata
  projected short — prefix display at the projection layer. RDF-style
  namespace machinery at the data layer is the failure mode to avoid.
- Name stays a plain string edge for now; icons and multiple languages
  arrive later as more name-shaped data, never as data-layer features.
- The editor never displays raw UUIDs; identicons were their visual
  form (superseded — see the identicon paragraph's deletion note),
  with a bijective encoding — all 128 bits recoverable from a
  rendering at the small standard size, which rules out sub-perceptual
  geometry (a v5 flaw: sub-pixel jitters rasterize away). The v6
  encoding (2026-07-05) is a nested mosaic with salience decaying
  exponentially from low bits to high — the git-short-hash principle
  made literal. Low bits pick a family (an outline plus its natural
  subdivision vocabulary: grids for square shapes, radial for round
  ones), base hue, saturation, and the level-1 division pattern; each
  of the four level-1 regions carries a hue transform and a split
  variant; sixteen level-2 regions carry lightness levels; sixty-four
  leaf tiles carry one-bit lightness offsets, tiling the icon with no
  blank space. Finer levels derive their colors from their parents, so
  the hierarchy reads top-down and decodes bottom-up; grout between
  level-1 regions and the frame show the raw base color, which anchors
  the decode. Every bit lands in a region of cell scale or larger.
  Refined to v7 the same day: colors moved to OKLCH so the lightness
  channel reads uniformly across hues (also the aesthetic fix — HSL's
  perceived-lightness swings were the garishness), hue transforms
  became analogous-leaning with one complement accent and a neutral,
  families were made geometrically coherent (grids live only inside
  grid-friendly outlines — sharp square, rounded square, and the
  diamond, whose grid rotates to fit exactly; round and pointy
  outlines subdivide radially), and frames stroke the family's actual
  outline instead of a generic rounded rect. Families that share an
  outline or a vocabulary carry constant structural signatures (a mat,
  a center hole) so no two families can render alike under any bit
  values; radius splits are equal-area so inner leaves stay readable
  at the standard size; out-of-gamut chroma is walked in rather than
  RGB-clamped so the lightness channel never distorts. The radial
  engine works in boundary-normalized polar coordinates — radius is a
  fraction of the distance to the outline in each direction — so
  wedges reach into a shape's corners (the shield's shoulders and
  point, the hexagon's tips) instead of stopping at an inscribed
  circle and leaving those margins blank.
  Tuned same day against rendered sample sheets: base hues moved from
  uniform 45° steps to eight color-name anchors, since glance identity
  is verbal ("the red one") and arithmetic steps land between names;
  the transform vocabulary tightened to ±45° plus a single complement
  accent after the sheets showed quadrant transforms out-shouting the
  base hue (an icon must read as its base category, with the accent as
  a feature, not a takeover); grout and outline chroma rose so small
  renderings still carry the hue; the offset magnitude moved below the
  leaf signs in bit order, being global texture contrast and thus more
  salient than the per-leaf noise floor above it; and the offset
  lattice was respaced so the sum of any two magnitudes stays clear of
  the level-2 spacing — no leaf value under one magnitude reads as a
  value under another.
  A second same-day pass traded the leaf-tile quilt for solid level-2
  panels carrying four small offset dots each: all-over tiling read as
  uniform texture — all ground, no figure — while panels give the
  family and quadrant structure legible area and turn the leaf bits
  into ornament. The shell gained a fixed-seed identicon sample sheet
  so palette and salience changes are judged in the app against the
  same identities; its temporary `i` toggle was removed once the
  tuning pass settled — raw-key commands don't belong in the shell —
  leaving the sheet module unwired until the next pass.
  Rebuilt same day as v8, a badge grammar, once the panels pass made
  the deeper flaw visible: the encoding was one fixed record, so every
  icon was the same subdivision with a different outline, and no bit's
  meaning ever depended on an earlier bit — the missed half of the
  hierarchy idea. Now the low bits choose a silhouette family, a
  family variant (sharp/rounded square, disc/annulus, diamond/shield,
  flat/pointy hexagon), hue, chroma, and a layout whose vocabulary is
  family-specific (grids, bands, and nested rings for squares; wedges,
  rings, and ring-crosses for the radial shapes). Four region records
  each carry a hue transform, a panel lightness, and an inner figure —
  disc, ring, diamond, square, bars, plus, saltire — with tone, size,
  aspect, and offset nuances: shapes inside shapes, with no "none" in
  any vocabulary so every field stays visible under every branch (the
  bijection rule for grammars; a hidden-when-absent attribute would be
  undecodable). Concentric-ring regions anchor their figures on the
  ring at twelve o'clock rather than stacking them at the center for
  the same reason. The high 64 bits became a beaded rim — 32 beads at
  four lightness steps on a dark band that follows the silhouette via
  the boundary-normalized radius — ornament rather than noise. Bit
  positions are static; only vocabulary is branch-dependent, so decode
  reads the branch bits first and the fixed-shape records after.
  A fix pass after review at size: figures are sized by their region's
  inscribed circle and always centered — a wedge tapers, so sizing by
  its mid-arc span had figures crossing their containers near the
  point; containment is now property-tested across every family,
  variant, and layout. The figure aspect and offset nuance bits were
  dropped as sub-perceptual at the standard size: the bijection rule
  cuts both ways, a field too small to read being as broken as one
  hidden. The freed bits became four figure sizes and fewer, larger
  beads (34 x 2 bits).
  Benched 2026-07-06: the raw view trials git-style id suffixes (an
  ellipsis and the last five hex digits, fixed length) instead —
  identicons added visual noise and required explanation, and
  truncated ids are precise, quiet, verbally communicable, and
  familiar from git. Deleted 2026-07-07: the graph view (their
  anticipated home) trialed them for unnamed nodes and the verdict
  was they are not missed anywhere — both views speak one identity
  language, short ids. The identicon and sample-sheet modules are
  gone (git history keeps the v6 encoding); "never displays raw
  UUIDs" is repealed, truncated ids being the display form.
  Separately: node-space minting dropped RFC 4122's version/variant
  structure (2026-07-05) — payloads are 16 raw CSPRNG bytes with full
  128-bit entropy, since the RFC structure exists to let different
  generation schemes share a namespace, a context the space doesn't
  have. That made "UUID" a misnomer, so the concept is named NodeId
  (the node space, `new_node_id`, a `"node"` serialized tag); the uuid
  type and its hyphenated spelling remain as tooling. The old NID
  abbreviation for numbers was a JavaScript-era artifact (the type was
  `number`); the Rust surface says node id, string, number plainly.
  Identicons are not spoof-resistant (salient features can be ground
  for); visual authentication would be a different tool. An explicit
  reveal command may show the text form later; it is never the
  default.

## Selection: Splice

Selection is (location, mode): the node at a path, or the gap between
that node and its parent.

- Gaps subsume list insertion points: between items, before the first,
  after the last, and the sole point in an empty list. Whole-list
  versus before-first-item stops being ambiguous. With
  ordered-identity lists a gap is directly representable by its
  neighboring positions, and committing an insertion is
  `between(prev, next)` plus one edge set.
- A splice names the gap at a path like any selection; stability
  across sibling edits is the general path-adjustment problem below,
  not a special splice property.
- Edge gaps generalize beyond lists: a splice on any edge is the
  wrap/insert-around position.
- This replaces list-specific selection variants from earlier
  prototypes. Projections still own the mapping from displayed gaps to
  graph edits; the displayed gap and the graph gap differ for lists,
  and that translation is list-projection policy.
- The underlying observation: a text cursor is a splice by default,
  which is much of why text editing feels fluid and node-only
  structural selection feels clunky. Splice imports the between-ness
  honestly.
- Paths stay plain label sequences. Decorating path elements with the
  value ids seen (cell anchors) was considered and rejected
  (2026-07-06): an id detects drift but does not correct it, and the
  same staleness lives in every path-keyed state — delete the first
  list item and a collapse override on the second-to-last now
  collapses the last — which selection-side anchors cannot touch. The
  honest mechanism, when list editing demands it, is general and
  structured: a structural edit adjusts all path-keyed state
  (selection, collapse overrides, whatever joins them) through one
  rewrite, the way text editors adjust marks. Ordered-identity lists
  (2026-07-06) then dissolved the list instance of the staleness —
  sibling edits no longer move any address — leaving wraps and
  unwraps as the rewrite's remaining customers. The label sequence
  remains projection identity — the same node projected at several
  paths is distinguished by path, never by id.
- Demoted 2026-07-06 by ordered-identity lists: gap selection is no
  longer required machinery. An insertion is a command relative to
  the selected item — the gap's graph side is `between(prev, next)`
  plus one edge set — and the displayed gap finally equals the graph
  gap, retiring the TypeScript prototype's custom insertion-point
  translation. The between-ness argument (a text cursor is a splice)
  survives as a UX option if item-relative insertion ever feels
  clunky; adding it back would be a pure selection mode with no model
  implications.

## Editing

- Modeless (2026-07-05): every string value is a text editor.
  Selecting a string edge focuses it — the editor state (cursor, text
  selection, drag) lives inside the app's `Selection` value, created
  when the selection lands on a string and dropped when it moves away,
  so there is no begin/commit mode and no parallel pending-editor
  state to sync (the TypeScript prototype's pending editors were a
  standing bug source). The graph stays the source of truth: edits
  write through after every handled event, retargeting the edge to the
  new string id — strings are values, so retargeting leaves no
  garbage. Unselected strings render as plain glyphs, which is exactly
  what an unfocused default-state editor draws. Clicking a string
  selects it and lands the cursor where the click did: the projection
  only reports what happened (this path, this text-local position),
  and the shell's single selection transition consumes it — creating
  or keeping the editor and advancing its caret — the same
  one-event-one-transition shape as the Haskell LineEdit's
  focus-with-initial-selection callback, moved into the shell because
  parley's editor hit-tests behind its measurement caches rather than
  exposing caret geometry as data. Pressing starts the editor's drag
  state, so drag-selection works from the selecting click. An editor
  mounted without a text click — a label click, a keyboard landing —
  starts with the caret at the end, so typing appends (2026-07-07; a
  select-all trial was rejected the same day: visually heavy and one
  keystroke from wiping the value). The rule is uniform over the
  already-selected edge too: re-selecting without a text click — its
  label, its chrome — lands the caret back at the end. With true-state
  custody this is a plain field write at construction; a briefly-lived
  deferred-seed mechanism (needed while the retained PlainEditor gated
  caret writes behind its driver) dissolved the same day it was built
  when the editor went transient. Arrow keys
  inside a
  focused string go to the cursor (up/down still navigate, since a
  single line declines them); IME plumbing beyond what the puri widget
  already carries (window enabling, candidate positioning) is
  deliberately deferred. Double-click selects the word and triple the
  line (2026-07-07): ui-events-winit's TapCounter supplies click
  counts through the existing reducer — no shell counting — and drag
  anchoring is gesture data (origin point + count) replayed each
  move, since only byte offsets round-trip the transient editors.
  Known accessibility gap, deferred to the accessibility pass:
  TapCounter hardcodes the 500ms interval and slop radii, ignoring
  the OS double-click-speed setting. Verified against winit 0.30:
  winit exposes no click counts, no double-click events (Windows
  CS_DBLCLKS unset), no interval setting — it discards the
  OS-computed NSEvent.clickCount already present in the mouse events
  it handles, which is why ui-events-winit recounts above it with
  hardcoded thresholds. Upstream state (checked 2026-07-07): winit
  #642 (2018, punted to applications) and #3899 (2024) — the latter
  proposes counts on pointer events with winit-maintainer and
  Linebender convergence (Option<NonZero>, macOS from
  NSEvent.clickCount, Windows from GetDoubleClickTime +
  GetSystemMetricsForDpi, hardcoded defaults elsewhere — Linux has
  no native setting), unimplemented; ui-events intends to adopt
  winit counts once they exist, so TapCounter is itself a stopgap.
  Fix routes when taken up: implement #3899 (whole-ecosystem, the
  consensus already exists); interim TapCounter config upstream; or
  ~40 lines of shell counting against NSEvent.doubleClickInterval
  (objc2-app-kit is already in our tree via winit) fixing only us.
  Numbers edit the same way with
  parse-gated write-through (2026-07-06): the editor may show a
  half-typed `3.` while the graph keeps the last parsed value, since
  an unparsable number has no identity to write; the edited kind
  follows the graph value, so digits typed into a string stay a
  string. The bufferless Haskell shape landed (2026-07-07): the
  selection stores only true state — base text, anchor/focus byte
  offsets, any active IME preedit — and a transient `PlainEditor` is
  constructed from it and discarded, per pass for drawing and per
  dispatch for editing semantics. Parley's retained-mode machinery
  (internal layout cache, dirty flag, driver-gated writes) lives and
  dies inside those moments; its debugged editing behavior is
  borrowed whole rather than re-owned; IME preedit stays out of the
  base text, applied as a pure state transition (keys and pointers
  already decline while composing), so the graph never sees
  composition. Deliberately not round-tripped, as
  single-line-irrelevant: cursor affinity, word/line drag anchors,
  the vertical goal column. This also retired the shell's
  refresh-before-pass step — layout freshness is now internal to the
  transient construction. Custody is one working copy, one direction
  — the write-through runs once after each handled event — and
  nothing else writes the graph today. Standing invariant for when
  something does (undo arrives first): any non-editor write to the
  currently edited edge must re-mint the editor state (`set_text`
  clamps the selection to the new text), or the stale text clobbers
  it. Sequencing (2026-07-06): dirty tracking waits for undo —
  modified-since-save falls out of history position — and undo waits
  for structural editing to exist; neither is built standalone.
- Undo landed (2026-07-07): a snapshot stack over the persistent
  gid — Document clones are O(1) structural sharing, and History is
  a dumb stack: record/undo/redo plus a saved position, no policy.
  Recording is explicit at each mutation site — deletes, value
  commits, picks, and graph detachments each record one step; a
  label commit records only when it mints (and names) a new node.
  Text-run coalescing is editing custody, not history's: the run IS
  the mounted editor's lifetime (`Selection::Edge` carries a
  `recorded` flag; `write_through` returns true only for the run's
  first real write, so the step opens at the run's start and later
  keystrokes stay silent), and saving breaks the run at the
  selection so it never straddles the mark. A corollary better than
  the interim design: revisiting an edge after selecting elsewhere
  re-mints the editor and is a new step by construction. The design
  arrived by successive user correction the same day: first
  ptr_eq-diffing across dispatch with selection-shape run inference
  (working backwards from the event stream), then declared step
  kinds with coalescing inside History; both replaced — handlers
  mark their own undo frames, and run identity lives with the
  editing session that owns it. Push-before vs push-after is a
  recorded non-question: "save what I'm displacing" and "checkpoint
  what I just made" are duals (same state sequence, cursor off by
  one), neither provably better; before-push won here purely on
  plumbing — mutation sites are natural narrators, while session-end
  (the after-push moment for text runs) has no single line of code
  in this shell.
  Dirty IS history position versus the save mark, so undoing back to
  the mark is clean again; the title carries a dot, and New/Open
  confirm before discarding. Undo restores the snapshot's document
  and re-mints the selection from it — the standing invariant's
  first client. Cmd+Z / Shift+Cmd+Z through the Edit menu. Quit
  remains unguarded (window-close interception deferred).
- Deletion (2026-07-06): Backspace or Delete removes the selected
  edge — detachment, the value staying for the orphan pool. A focused
  atom editor claims the keys while it has text and declines on an
  empty buffer (a no-op edit is not a handled edit), so emptying a
  string then backspacing again deletes the element — the text
  editor's join idiom, modeless like everything else. Selection lands
  on the next sibling, else the previous, else the parent. The root
  is a location like any other — the empty path commits to the
  document's root field — so deleting it empties the document
  (`root: Option`, rendered as a selectable placeholder) and an atom
  root is editable like any atom. The TypeScript prototype threaded
  per-descend commit closures for the root and for list gaps;
  ordered-identity lists removed the list motive, leaving the root as
  the one special-cased commit.
- Secondary selection (2026-07-06): the identity at the end of the
  selected edge marks each of its other projections with a subtle
  wash over the whole projection — expanded block, collapsed header,
  atom text, or label — while the selected one carries only the
  primary highlight. Uniform across spaces: strings and numbers are
  identities like any GUID node (a first cut marked GUIDs only, but
  every `"origin"` is one SID and every `2` one NID — the marks
  surface exactly that sameness, labels included). Derived in the
  pass from the primary selection each frame, so there is no second
  selection state to synchronize (the TypeScript prototype's
  secondary-selection sync bugs dissolve into a derivation).
- Insertion (2026-07-06): raw's insertion is edge insertion,
  uniformly. A node is a bag of labeled edges, so Enter begins a new
  edge on the selected node — falling back to its parent when the
  selection is an atom — and Cmd+Enter targets the parent explicitly
  (swapped from parent-first 2026-07-07: targeting the parent sent
  the pending row to the bottom of large blocks, visually far from
  the selection; into-the-selection keeps authoring where you look,
  and ordered after-current insertion remains the future list
  projection's Enter). Edges author in two stages
  through the same completion: label first (string labels for the
  casual gradient, references for schema-shaped GUID labels, a fresh
  node minted on the spot), then value; resolving a label that
  already exists selects the existing edge instead. The new edge
  appears wherever its label sorts — raw makes no ordering promises.
  A first pass minted position labels from the gesture (sibling
  after/before, append into, Shift and Cmd variants), which was list
  semantics leaked into the semantics-free layer; those gestures are
  parked for the list projection, which owns element insertion —
  ordering is a list concept, so Shift+Enter retires from raw with
  them. On an empty document Enter begins the root value with no
  label stage — the root is a value slot, not an edge. The pending
  renders in place with the query editor focused — a value stage
  sorted among its would-be siblings, a label stage unsorted at the
  bottom of its node until it has a label to sort by; Enter resolves
  the query to an identity (quotes force a string, parsing text is a
  number) and commits in one write. Discard is disposal of selection
  state: Escape clears the selection outright — from pendings and
  plain selections alike (2026-07-07) — empty-Backspace cancels a
  pending back to its anchor edge, keeping the keyboard flow, and
  navigating or selecting away simply replaces it. The ranked completion popup rides both
  stages' queries (2026-07-06): the universal layer as entries — the
  inferred atom, node references (fuzzy-ranked, short ids as
  detail), and a fresh node named after the query
  (create-on-reference). The atom leads only when the query states
  atom intent — a leading quote (string mode holds while typing,
  before the close) or a numeric parse; otherwise confident
  reference matches (prefix/substring tiers) outrank the typed
  string, which sits above only fuzzy matches — typing a visible
  name or short id defaults to the reference, and quoting always
  forces the string. The typed text is always insertable as itself
  (2026-07-08, user: typing "5" offered no way to insert the string):
  a numeric query offers its string form directly below the number;
  a quoted query stays string-only, the quote being stated intent. Entries are drawn by the shell
  after the body from the frame's popup output, driven by Up/Down and
  committed by Enter or by clicking a row — the card swallows other
  clicks so nothing lands on content underneath. Polish (2026-07-07):
  the card flips above the anchor when it would run off the bottom
  and fits above (the TS behavior); rows pad out to the widest so
  the chosen highlight spans the card; the filter's match spans draw
  in bold — byte offsets rendered, never recomputed. The shell also
  scrolls to reveal on selection change — the popup anchor while
  pending, the selection's rect otherwise — once per change, so it
  never fights manual scrolling; computed from the post-event
  dispatch pass before anything draws, so the reveal lands in the
  presented frame (a first cut revealed after presenting, a visible
  corrective flash), with the pad as landing margin, not trigger. Vertical arrows
  belong to the popup while pending, so discard is Escape,
  empty-Backspace, or selecting elsewhere. Command-click picks by
  pointing (2026-07-07, the TS/Haskell choose-id gesture): with a
  pending open, command-clicking any projection of an identity — a
  tree value or label, a graph node or pill — commits it into the
  open stage, value or label alike; with nothing pending the
  modifier is inert and the click behaves normally. Selecting the empty
  document's root pends immediately — there is nothing there to
  select, only something to begin — with Escape deselecting rather
  than re-pending.

## List Projection (Design Brief, 2026-07-07)

Written at the end of the session that built the editing middle-game,
so the next session starts warm. The list projection is the first
convention-aware layer over raw and the first test of projection
layering; it restores the ordered-element ergonomics raw deliberately
lacks (the 2026-07-06 correction parked `pending_after`/`before`/
`into` in raw.rs, tested, behind `#[allow(dead_code)]`, for exactly
this).

- Recognition: lean toward PARTITION, not all-or-nothing — split a
  node's edges into position-labeled (the ordered elements) and the
  rest (fields, rendered as raw rows above). A list carrying a `name`
  or other metadata then degrades gracefully instead of cliffing back
  to raw. All-positions-only is the simpler v1 fallback if partition
  rendering fights the layout.
- Layering mechanics (the real architecture question): v1 is a
  hardcoded chain in `value_view` — list-shaped → list view, else the
  raw block — decided per node. A projection registry waits for
  user-defined projections. Paths are untouched: position labels are
  real labels, so selection, undo, secondary marks, the graph view,
  and persistence all work unchanged underneath.
- The projection must be queryable at dispatch, not just at render:
  `insert_key` needs "is this path list-shaped" to pick gestures, and
  that is a pure doc question (inspect labels), needing no frame
  state.
- Rendering: elements as rows WITHOUT their position pills — the
  position is identity, not information; order carries it. Fields (if
  partitioned) render as ordinary raw rows. Inline `[a, b, c]` for
  short atom-only lists is desirable but Wadler-style grouping can
  wait; v1 is block rows. Collapsed form shows an element count.
- Gestures: Enter on an element = pending sibling AFTER it
  (`pending_after`); Shift+Enter before; Enter on the list node
  itself = append (`pending_into`); Cmd+Enter stays raw's
  edge-on-selection as the escape hatch (adding a field to a list
  node). Elements are ordinary value pendings — one stage, no label
  query, since the projection mints the position.
- Completion: elements use the universal layer as-is; the projection
  contributes no extra offers in v1 (the projection-parameterized
  layer stays future).
- Possibly riding along: name-awareness (a named node showing its
  name as header) is the same kind of convention-awareness — decide
  in-session whether it is part of this layer or its own.

Shipped 2026-07-07, same day. What landed matches the brief:
partition recognition (`list_shaped` = any position-labeled edge, or
a pending one, so a named list renders its fields as raw rows above
the elements — the sample's `points` list is named to show this); the
hardcoded chain in `value_view`; elements as bare value rows with
positions suppressed; collapsed form shows the element count; Enter
on an element pends after (Shift+Enter before), Enter on the list
node appends (Shift+Enter prepends — added for symmetry), Cmd+Enter
is the field escape hatch. The gesture chains live in raw
(`pending_enter`, `pending_enter_before`, `pending_field`), not the
shell, and the dispatch-queryable-shape requirement dissolved: the
parked pendings gate themselves, so the chain just tries them in
order. Deviations and decisions to watch: (1) Enter on an EMPTY node
takes its first element — this is how lists begin, and it deliberately
outranks the sibling gesture so an empty node nested as an element is
fillable at all (a keyboard path to `[[1,2],[3]]`); fields on fresh
nodes take Cmd+Enter. (2) Cmd+Enter became SELECTION-first (it
targeted the parent first before; the parent fallback remains for
atoms) — its old parent-targeting was a stopgap for lists, which the
real gestures replace. (3) Enter on a node-valued element pends a
sibling, not a field on it — fields on elements take Cmd+Enter.
Name-awareness was deliberately left out: it is its own convention
layer, not part of this one. (Recognition superseded 2026-07-09:
lists joined the data model, so `list_shaped` is a kind lookup, the
partition's fields side became unrepresentable and was deleted, and
lists carry no name edge — see Lists.)

The convention knows its own node (2026-07-08, user-reported: File >
New, make a named node, and the graph view showed `…fed8` pills —
the "name names itself" edge had been sample-document DATA, not
editor behavior). `Names::convention()` now answers for the NAME node
itself — text "name", `label: None`, nothing consumed, nothing
pretending to be an editable edge — with a stored name still winning.
The sample's explicit self-name edge is gone as redundant (its
floating name → "name" pair leaves the sample graph). Knock-on win:
completion now offers "name" as a key wherever the NAME node appears,
so naming an existing node by keyboard is Enter, type "name", pick,
type the name — no hex required. The eventual home for facts like
this is a LIBRARY gid layered under the document (StackedGid has
waited for exactly this since the egui era); the fallback is the
editor-convention stopgap, deliberately not that architecture step.

The library step was then taken the same day (user direction, with
three corrections from prior prototypes baked in). `Sources` is the
reading context — `{ doc, library }`, two gids held EXPLICITLY,
deliberately NOT implementing Gid: past hybrids merged the layers
into one graph view and then had to bolt provenance metadata back on
for read-only UI; when the combination never masquerades, provenance
is simply which side answered. Presentation (Cx, graph content,
labels, the name policy, completion) reads through both; MUTATION
resolves against the document alone, so library facts are read-only
with no flags anywhere — editors don't mount, deletes decline,
write-through drops. Fallback is per ENTITY, never per edge (user:
an edge-level merge tempts inheritance at the data layer; semantics
like that belong above the substrate). `conventions::library()` names
the well-known ids, replacing the hardcoded NAME fallback with data.
A first cut also named the four identity-space uuids; removed the
same day (user: spaces are the id MECHANISM, external to the graph —
not graph vocabulary). An `isa`/`List` pair minted in their place for
the 2026-07-06 empty-list marker lived one uncommitted day before
lists joined the data model (2026-07-09, see Lists) and shrank the
library back to `name`. Completion sweeps both layers, so
the conventions are typeable from keystroke one and picking "name"
yields the NAME node rather than a lookalike string label (the fresh-
document trap). The graph pane's SNAPSHOT stays document-only —
library facts enrich display, they don't populate the picture.
StackedGid retired from progred_graph. One hole surfaced by use the
same day (user: can't edit the name node's name, but CAN add it an
arbitrary edge — intentional?): resolution-gating protected library
EDGES but not library ENTITIES — the entity's id resolves through any
document reference, so pending gestures would open on it and the
committed edge, landing in the document, would SHADOW the library's
whole entity (per-entity fallback), silently de-naming the
conventions — fork-on-write through the back door. Closed by the
previous prototypes' rule: the whole library node is read-only —
`pending_edge`/`pending_into_at` take the library and decline on
entities it describes, while referencing them and authoring beside
the reference stay ordinary document edits. The editability conflict then resolved in conversation (user: "we
should be ABLE to make any kind of edit... the fractional ids also
complicate that"): the substrate stays fully permissive — a document
may state anything about any identity — and read-only is purely an
editor affordance above it. Under per-entity fallback, the only
COHERENT edit of an external entity is a whole-entity fork (a partial
edge would shadow the rest away; a library list makes this vivid —
one inserted position edge would leave a one-element list, so the
fractional design and the per-entity design agree the entity is the
unit). The fork needs NO command: it is copy/paste's job (user call —
rare on purpose, possible by design), so the gates now check
AUTHORITY, not description: `Sources::external(entity)` = the library
describes it and the document does not; external entities render on a
subtly darker ground (tree wash, graph node fill, label pills — no
lock iconography, per user) and decline authoring, while a document
that owns the entity — today by hand, eventually by paste — authors
freely, its copy diverging from the library thereafter, which is the
semantics of taking a node over. Authority is per-ENTITY everywhere — settled after two user probes
(a library node pointing at a document node "should have a light bg
again"; fork-something-inside-a-library-structure "we should be able
to edit the thing we just forked"). A first cut had the tree gate by
PATH (mutation resolved document-only, so everything past a library
hop was inert), defended as honest-affordance — but entity-level
shadowing already made the CONTENT under a library hop show the
document's version of a forked entity, so path authority was the one
layer disagreeing with the other two. Now: `resolve` reads through
the Sources (navigation reaches what presentation shows), every WRITE
gates on the parent entity's authority (`writable` = not external;
editors only mount where write-through can land), and the ground
follows the same rule — external entities on the dark tint, a
document-authority entity nested under an external ancestor gets an
opaque light patch back, editable right there because it is. The fork
flow: copy the entity to anywhere in the document (identity-
preserving paste — no dedicated command, rare on purpose), and the
library structure immediately shows and edits your version in place;
the structure node itself stays external and inert. Grounds paint
only at authority TRANSITIONS (user simplification — the first cut
painted by absolute state, which double-tinted nested externals and
repainted light redundantly): a node draws a ground iff its authority
differs from its parent's — dark entering external, opaque light
leaving it — so runs of the same authority draw nothing and nesting
never stacks. One parent check replaced the ancestor walk.

The (doc, library) pair then got its name (user call): `Sources`
grew to hold the `&Document` itself, and every reader — `resolve`,
`root`, `external`/`writable`, edges — is a method on it. One value
travels through the whole read side (Cx dropped its separate root
field; completion dropped its root parameter; project and the graph
pane take `&Sources`; the shell builds it via `Model::sources()`).
The four WRITERS (write_through, delete_edge, set_value,
commit_pending) keep explicit `(&mut Document, &MutGid)` — mutation
needs the exclusive document, so the bundle cannot carry it — and
construct a Sources internally for their read phase. Multiple
libraries stay future-simple: being read-only, they compose UPSTREAM
by merging into the one `Model.library` gid (`MutGid::merge` has
waited for this); `Sources` remains two-sided permanently. Open:
copy/paste itself; user-loadable libraries and the completion noise
budget (five convention offers on an empty query) are future calls.

Label pendings own their clicks (2026-07-08, user-reported: Enter on
the root list, then clicking the header kept the pending instead of
selecting the node). There is no quasi-selected state — PendingEdge
deliberately leaves its parent unmarked and the row carries the
primary — but PendingEdge has no path of its own (`path()` names its
PARENT), and the selection transition's "same path keeps the mounted
editor" guard conflated the two. That conflation was also accidentally
load-bearing: it was the only thing keeping a click on the pending row
from discarding it. Fix is two-sided: the pending-edge row swallows
its own pointer-downs (nothing on it means "select the parent"), and
the transition treats PendingEdge as never path-equal, so a reported
click is always a real selection change. Riding along: query editors
(both pending stages) gained click-to-caret, wired straight through
the edit hook — the selection transition is never involved, so
clicking what you're typing can't discard it.

Gestures settled on BESIDE/WITHIN 2026-07-08, superseding the same
day's within-first assignment (below) after one more real-use bump —
the two consistent polarities were laid out and the user picked
beside-first to try. Plain Enter continues the enumeration you are in
(the outliner convention): any element, atom or node, pends a sibling
(Shift+Enter before); a field value pends the parent's next field;
the root, with nothing beside it, takes the field on itself.
Cmd+Enter authors within the selection: a field edge on the selected
node — lists and empties included, so list metadata is reachable —
and Cmd+Shift+Enter is the positional variant, a first element at the
front: prepend on a list, and how an empty node becomes a list
(nested-as-element included). Shift consistently means the
front/before flavor. Atoms decline the chord (no within). The one
loss, accepted: no append gesture on the list node itself — append is
Enter on the last element; `pending_into` is parked for its return.
No carve-outs remain: element-vs-field behavior follows position
uniformly for atoms and nodes alike.

Amended when lists joined the data model (2026-07-09, see Lists): a
list holds no fields, so "within" on a list means its elements —
Cmd+Enter appends (the parked `pending_into` returned for exactly
this), Cmd+Shift+Enter still prepends, and the field pending
declines lists outright (`pending_edge` gates on kind, so committing
can never hit the data layer's kind panic). Enter on a root list
falls within the same way: append, where a map root takes a field on
itself. List metadata via fields is gone with the mixed entity —
metadata belongs on a wrapping map or the referencing edge. (The
empty-node-becomes-list arm of the shift chord died the same evening
with the typed model — lists begin as the "new list" completion
offer; see Data Layer v2.) The
name machinery came out of `list_view` the same day (user: lists
won't have names; remove what assumes they will) — the list header
is always the short id, and the inline literal never leads with a
name. Completion's "new list" was re-cut from an always-trailing
offer to a RANKED entry (user): it competes under its own display
text like any reference — type toward it and it surfaces (a "new
li" prefix match leads), type away and it leaves — so the popup
carries no permanent extra row, easing the parked noise budget.

Gestures REASSIGNED 2026-07-08 after real use (user: Enter on a node
element should add an edge to IT — and "cmd+enter doing the list
thing" was their original instinct, which the brief had flipped).
Enter now authors ON the selection: a new field edge on any node —
records, lists, empties, node elements alike; atoms keep the one
carve-out they force (no edges to author): an atom element pends a
sibling (Shift+Enter before), any other atom defers to its parent's
field, so the fast `2, Enter, 2.5` flow survives unchorded. Cmd+Enter
is the positional gesture, in list terms: a sibling beside any
element (Cmd+Shift before), append/prepend into a list, and a first
element into an empty node — still outranking the sibling so nested
empty lists stay fillable. Consequences: the field escape hatch
dissolved into the main gesture (a list node takes fields with plain
Enter), inserting beside a NODE element is now chorded, and "how
lists begin" moved to the chord — mint a node, Cmd+Enter, type. The
chains are `pending_enter`/`pending_insert` in raw, shift folded in
as a flag; `pending_enter_before` and the standalone escape-hatch
role of `pending_field` retired.

First-launch feedback ("basically the same as before but w/o the
labels?... I was expecting [x, y, z]") pulled two things forward the
same day. (1) A fieldless atom-only list projects INLINE as
`[1, "two", 3]` — dim brackets and commas, every element still an
ordinary descend (click, edit, secondary-mark), a pending element's
query editor sitting inline between the commas. The inline form drops
the short-id header (that is the sugar); fields or node elements keep
the block form, whose element rows now carry a dim leading `-` — the
YAML vernacular — so they read apart from labeled field rows.
Width-aware inline-vs-block choice is still the Wadler layer; until
then a long atom list runs wide. (2) A slice of name-awareness
arrived early, for LABELS only: a named GUID label renders its name
in label style (graph pills and completion already did), and the NAME
convention node names itself "name" in the sample, so `name` rows
finally read `name → "polygon"` instead of a hex id. A named GUID
label and a SID label are now visually identical — the identity
difference shows only through secondary marks; distinguishing them is
an open styling question. Value-side name-awareness (a block headed
by its name) stays its own layer.

Names became POLICY immediately after (user direction: "names are a
bit special and have deep but still configurable editor support").
`conventions::Names` is one editor-state function
`(&MutGid, &Id) -> Option<Name>` — `convention()` reads the `name`
edge, `none()` disables names for a strict raw view — owned by the
Model (an editor setting, so it survives document swaps), threaded
through the tree's `Cx` and the graph pane, and every display-name
check asks it: tree labels, completion keys, graph node/pill content.
Expanding what a name is (computed names, per-library conventions) or
turning names off is now a swap of that one value; a View-menu toggle
for `none()` is the natural next consumer. The WRITE side —
completion's name-your-new-node offer and mint — deliberately stays
hardwired to the `name` convention until it needs to vary.

Refined immediately (user: a lookup alone is not enough — the
projection must also know to SKIP the name field below): the policy's
answer is `Name { text, label: Option<Id> }` — the text plus the edge
it CONSUMED (`None` for future computed names, which consume
nothing). Name-aware blocks project the consumed edge AS the header:
named nodes and lists head with their name (near-black, replacing the
short id — the graph view's language) and skip that edge in their
ordinary listing. The header name is the name edge's projection, not
a copy of it — it descends, selects, click-places the caret, edits
with write-through, secondary-marks, and deletes exactly like the row
it replaced; deleting it reverts the head to the short id. A named
fieldless atom list stays inline, the name leading the brackets:
`pair [1, 2]`. Header clicks are TWO-STAGE (user: "how do I select a
node now that the header is a text box?"): the name text stands for
the node until the node is selected — a cold click falls through to
the block's target and selects the NODE; once the node (or the name
edge) is selected, the name engages as a text target and the next
click selects the name edge with caret placement. Click to select,
click again to rename — the Finder pattern. Implemented as a
pass-time conditional target (the pass reads the current selection);
single-shot dispatch guarantees the second click meets the engaged
successor, never a stale target. Cold, the name edge still descends
(keyboard-reachable, secondary-markable) — it just registers no
pointer handler. Tap counts never span targets (user correction —
stage one is selecting the node, not half a double-click): the report
stays honest (cursor_target passes the physical count), and the
shell's selection TRANSITION clamps to a first click whenever the
click is what mounted the editor; counts pass through only while the
same editor stays mounted (double=word, triple=line). This also stops
a quick click across neighboring atoms — inline list elements sit
within double-click slop — from misreading as a double-click in the
second one. Open questions recorded: the graph view still draws
name edges even though its node content shows the name — hiding them
there is a separate call — and value headers hide the id entirely, so
two same-named nodes read alike outside secondary marks.

View > Raw (Cmd+R) arrived as the policy's first consumer: a check
item like Graph, muda-owned state read at frame time. Raw is a VIEW
toggle, not a settings rewrite — the frame passes `Names::none()` and
bypasses the projection chain (one `!cx.raw` gate in `value_view`;
names go quiet through the policy with no extra plumbing) while
`model.names` keeps the configured policy underneath. Raw shows the
pure graph: short-id headers, name and position edges as ordinary
rows, no brackets or dashes. Two knock-ons accepted as coherent:
completion keys follow the policy, so raw completion offers nodes by
short id only; and the list GESTURES stay live (Enter beside an
element still mints a position) — gestures follow the document's
shape, not the view.

Redrawn when lists joined the data model (2026-07-09, user call):
lists render as lists in Raw too, brackets and dashes intact,
because kind is data, not convention; flattening a list to position
rows would be showing session-minted artifacts the file doesn't even
contain. Shape settled the same day (user): Raw is ONE BIT of view
state, threaded as itself — `Cx.raw`, and the same bit into the
graph pane and completion — with every name lookup DERIVING from it
(`Cx::name` answers None when raw; the graph's `content` and
completion's keys gate the same way). A first cut instead deleted
the flag and swapped `Names::none()` in at the shell; the user
called both halves: deleting was premature (domain projections will
stand down through this bit when they arrive), and the swap made
"which names function is active" a second piece of state — the
names function should be a function of the bit. `Names::none()` is
gone with the swap. An even-rawer all-space-and-bytes inspection
view (positions visible) remains a separate hypothetical, as the
module doc always framed it.

## Copy/Paste

Shallow shipped 2026-07-10 (user call: only the selected
value/identity, no recursion — deep copy waits on domain-specific
projections deciding the boundary, per the paste-axes design in Data
Layer v2). The clipboard carries ONE VALUE as text: atoms spell as
the query language — "quoted" strings, bare numbers — so they read
in other apps and round-trip exactly (the string "42" keeps its
quotes); nodes and lists spell as Value JSON. Paste is
try-Value-JSON-else-the-query-reading, so text copied anywhere
pastes sensibly. Copy takes the tree selection's resolved value or
the graph selection's node/edge value; paste goes into an open
pending first (both stages — the label stage narrows to atoms
through the same pick gate as command-click) and otherwise replaces
the selected edge's value as one undo step, remounting the selection
so a pasted atom gets its editor. Layering: the chord lives in the
shell's key FALLBACK, so a focused text editor's own Cmd+C/X/V wins
by dispatch order — and copy/paste are deliberately NOT menu items,
because muda accelerators intercept ahead of key dispatch and would
take the chord away from text editing everywhere. Consequences of
shallow: pasting a node reference aliases (no entity rows travel),
so pasting an external reference does NOT fork the entity — the
identity-preserving fork still requires deep copy, which arrives
with the projection-boundary design. Cut is deferred (copy plus
delete when wanted).

## Graph View

For demos on small graphs (2026-07-07), carried from the
TypeScript/egui/Haskell prototypes as one design — same force
constants (repulsion 8000, spring 0.02 toward rest 120, damping 0.85,
gravity 0.005, max force 10), same FNV-seeded deterministic initial
positions, same rendering (rounded-rect nodes: names for named,
short ids for unnamed (identicons were trialed here and deleted —
one identity language with the tree) — atom values as
their own nodes, so a shared `2` is visibly one node; quadratic edges
with arrowheads and label pills, cubic self-loops, parallel-edge
fanning; root tinted). Puri shape: positions, velocities, and drag
are explicit model state; the pane is one pure pass — build
geometry from state, draw it, register handlers over it — and the
simulation steps once per redraw, with the continuous redraw request
gated on the simulation being visibly in motion or dragged — unlike
the prior prototypes' run-forever loops, it sleeps when settled and
any event that changes the graph reheats it (View > Graph, Cmd+G).
Click-vs-drag slop is measured in panel pixels for nodes and pans
alike; a first cut measured node slop in world units, a
zoom-dependent dead zone that read as drag latency. The graph's
animation is also what exposed the shell's per-event pass rebuilding
dispatch handlers from state newer than the pixels — quick grabs on
a hot graph missed their node — settled by dispatching into the last
rendered frame's handler (see puri.md), which also deleted the pass
per pointer move. Selection storage (second pass, 2026-07-07): ONE
Model-level slot — `Selected::Tree | Selected::Graph` — replaced the
two per-pane fields after a review caught undo/redo restoring a tree
selection without clearing the graph's, breaking the exclusivity both
key handlers assumed. With one slot there is nothing to synchronize:
selecting in either pane inherently clears the other, and the graph
pane no longer owns a selection — it reports what a release was
(`Release::Drag`, `ClickNode`, `ClickBackground`) and the shell makes
the transition. The one selection mirrors across panes as secondary
marks both
ways, always through identity: the graph-selected node marks its
projections in the tree; the tree-selected edge marks its VALUE's
node in the graph, and pills wash only when their label is the
secondary identity (mirroring the edge itself as a pill wash was
tried and dropped 2026-07-07 — it read too much like a graph-side
edge selection). Selection vocabulary unified 2026-07-07:
the pane-local primary is strong (the tree's translucent blue fill
with a full-strength ring; the graph's full-strength blue stroke),
and the secondary mark is one shared treatment in both panes — a
translucent blue wash plus a thin translucent blue outline — for
identity occurrences. The two panes speak one language, primary
can't be mistaken for secondary, and the prototypes'
near-black/gray selection strokes are retired (a wash-only tree
secondary proved too subtle next to the graph's). Deletion follows the
TS/Haskell semantics — the egui port's was dead code — an edge is one
detachment; a node is detached everywhere (root cleared if root,
outgoing and incoming edges removed), with unreferenced values simply
dropping out of view. Viewport (2026-07-07): the pane clips; pan and
zoom are world-space model state — background drag pans, trackpad
two-finger scroll pans (ScrollDelta pixels, matching the document
scroll's sign convention), pinch zooms toward the cursor (winit
PinchGesture, handled outside the reducer, which doesn't cover
gestures), and wheel lines zoom toward the cursor; text re-lays-out
at the zoomed size, so it stays crisp rather than scaling glyphs.
Scroll over the panel routes to the graph ahead of the document.
List values in the graph (2026-07-09; final form same day after the
typed model's first cut drew a list as its inline literal and a
points list became one giant uuid blob — user-caught): a list is a
small square-cornered node (maps stay rounded — kind is data, worth
a silhouette) whose content is just the bracket mark (`[…]`, `[ ]`
when empty), fanning ORDINAL EDGES to its element nodes — 1-based
dim pills, the order shown structurally, positions never. Shared
values are walked once, so two equal lists are one node with one set
of element edges. Ordinal pills are display only — no selection, no
deletion — because the value is immutable and shared (removing "an
element" of a twice-referenced value is ill-defined); element edits
belong to the tree. Known cost of value identity: editing an element
mints a NEW list value, which reseeds at a fresh spot — the
spot-transfer continuity problem (already scoped out) now visibly
applies to lists. Still scoped out: position
continuity across identity changes (the Haskell spot-transfer).

## Types And Autocomplete

Deferred behind projections.

- Bootstrap: projection-owned completion, no schema required.
- Layered like rendering (2026-07-06): a projection contributes
  parameterized offers where it knows something, over a universal
  substrate layer available at every site — atoms by inference (text
  that parses is a number; quotes force a string), a fresh node, and
  name search across the document and orphan pool (isa shown beside
  matches for disambiguation), whose misses become
  create-on-reference. Every node is referenceable (2026-07-06):
  unnamed nodes join the search keyed by the short id they render as
  — what you see is what you can type — trailing named matches on an
  empty query, so anonymous nodes (lists included) can be aliased. Raw offers only the universal layer, so the
  root is not special: name-and-isa search works there like anywhere.
  Domain projections own their root offers (a fresh document of their
  kind — templates) later; shape-recognized projections bootstrap
  through the universal layer, since an empty root has no shape to
  recognize. Labels resolve through the same layer (2026-07-06): a
  new edge is authored label-then-value, both stages one completion —
  the contextual label gradient (SID strings casually, GUID
  references where schema-shaped) needs no separate machinery.
- Pending resolves identity (2026-07-06): no committing a string and
  reinterpreting it. The selection can name a nonexistent edge; the
  projection includes selected nonexistent edges as pending rows; the
  completion query lives in the selection the way the line editor
  does; resolution commits the chosen identity in one write, and
  deselection discards the pending row with the graph never touched.
- Fuzzy matching carries from the TypeScript prototype's
  compositional filters (`filters.ts`): tiers chained over rejects —
  exact prefix, exact substring, case-insensitive forms, then fuzzy
  subsequence — each tier sorted by fraction matched, match spans
  driving highlights. Ported as `progred/src/filter.rs`, with spans
  byte-correct under case folding.
- Later: a minimal schema in the shape the TypeScript prototype already
  proved sufficient (ctors/records, fields, sums, an expected
  element type for lists). Everything is tagged, so TypeScript-style
  structural description gets strong identity anyway.
- No general generics. The list case is covered by an
  expected-element-type node; a second generic container earns a second
  special case before it earns type application.

## Floating Definitions

- Create-on-reference: committing an unresolved placeholder mints a
  floating node with just a name (wiki red links). Duplicate names
  cannot collide, and two stubs discovered to be the same thing are
  merged — a refactor text cannot express. (2026-07-21: the popup's
  one-step named mint retired for uniformity — "new cell" mints
  bare and naming follows on the head; the doctrine's two-step
  spelling.)
- Nothing needs to live under anything; the graph allows floating
  nodes. Orphans are the pool: autocomplete searches it alongside
  module-scoped definitions, and a pool browser lists and manages it.
- Garbage collection is explicit only.
- Re-examined under the typed model (2026-07-10, the lingering-
  pure-graph sweep): the doctrine stands, with its justification
  corrected. Undo does NOT need detachment — history is snapshots of
  a persistent gid, so undoing a purge would restore the purged
  entities identically. What detachment actually buys: deletion
  stays one edge operation with no reachability policy in the hot
  path, and a detached subtree stays re-attachable through
  completion without rewinding history. The WATCH ITEM: the pool is
  invisible and serializes into every save, so files grow
  monotonically, and the data cannot distinguish a wanted floater
  from a dead draft — the pool browser above is the eventual answer
  (visibility plus explicit cleanup), not a save-time sweep. If a
  sweep is ever built anyway, reachability must count KEYS as
  references, or the floating field definitions it exists to protect
  (stroke-width) die with the garbage.
