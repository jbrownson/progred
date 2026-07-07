# Data And Editor Model

Date: 2026-07-03

## Data Layer: gid v1, Unchanged

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
  gracefully (tag identicon plus opaque bytes). Today's SID/NID enum is
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

- Encoded as ordered-identity labels (2026-07-06): a list is a node
  whose element edges are labeled by position-space values, and the
  labels' identity order is the sequence. Insertion mints
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
  share), range operations relabel O(n), an empty list needs a
  convention marker (nothing else distinguishes it from an empty
  record), and traversal is a sort the projection already does.
  Position-identifier schemes interleave oddly under concurrent
  merging; irrelevant single-user, remember it if collaboration
  arrives.
- The empty-list marker is an `isa → List` tag on an ordinary node
  (2026-07-06) — the anticipated isa convention's first customer. A
  reserved swap-in empty-list identity was rejected: lists are nodes
  and nodes are shared, so swapping the value at one referencing edge
  silently diverges every other reference. Projections may still
  infer list-ness from position-labeled edges when untagged; the tag
  decides the empty and ambiguous cases.
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
  citizens, and the difference should be visible (identicon-labeled
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
- The editor never displays raw UUIDs; identicons are their visual
  form, and the encoding is bijective — all 128 bits recoverable from a
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
  familiar from git. The code stays, the sample sheet still draws
  them, and the graph view is their likely home; "never displays raw
  UUIDs" is suspended for the trial, decided by dogfooding.
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
  state, so drag-selection works from the selecting click. Arrow keys
  inside a
  focused string go to the cursor (up/down still navigate, since a
  single line declines them); IME plumbing beyond what the puri widget
  already carries (window enabling, candidate positioning) is
  deliberately deferred, as is double-click word selection (the shell
  does not yet count clicks). Numbers edit the same way with
  parse-gated write-through (2026-07-06): the editor may show a
  half-typed `3.` while the graph keeps the last parsed value, since
  an unparsable number has no identity to write; the edited kind
  follows the graph value, so digits typed into a string stay a
  string. The editor's PlainEditor buffer duplicates the edited text
  while selected — parley's engine owns its working string — but it is
  a plain caller-owned value with no identity leaned on: rebuilding it
  each frame from the graph text plus bare cursor state (the
  bufferless Haskell shape) stays available if the duplication ever
  bites; Rust's methods-on-values packaging is the only real
  difference (2026-07-06). Custody is one working copy, one direction
  — the write-through runs once after each handled event — and
  nothing else writes the graph today. Standing invariant for when
  something does (undo arrives first): any non-editor write to the
  currently edited edge must re-mint or drop the editor, or the stale
  buffer clobbers it. Sequencing (2026-07-06): dirty tracking waits
  for undo — modified-since-save falls out of history position — and
  undo waits for structural editing to exist; neither is built
  standalone.

## Types And Autocomplete

Deferred behind projections.

- Bootstrap: projection-owned completion, no schema required.
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
  merged — a refactor text cannot express.
- Nothing needs to live under anything; the graph allows floating
  nodes. Orphans are the pool: autocomplete searches it alongside
  module-scoped definitions, and a pool browser lists and manages it.
- Garbage collection is explicit only.
