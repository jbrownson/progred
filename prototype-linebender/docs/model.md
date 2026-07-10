# Data And Editor Model

Date: 2026-07-03

## Data Layer v2: The Typed Model (2026-07-09)

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
lists = one node, value semantics displayed honestly), list nodes
draw square with their inline literal as content, position edges
don't exist so the ordinal pills went away entirely, and node
deletion detaches occurrences INSIDE list values too (a `without`
walk), the root list included. The one intentional main.rs seam:
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
  that holds the list plus its metadata), manual today, the reserved
  path-rewrite wrap gesture eventually. Kind CONVERSION has no
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
List edges (2026-07-09, with lists in the data model): the pill
shows the element's 1-BASED ORDINAL, dim like the tree's dashes —
the order is the data; the position bytes are its session spelling
and stay out of view here as everywhere. Computed per source from
the snapshot's sorted position labels; selection and deletion still
key by the real label underneath. List NODES draw square-cornered
where maps stay rounded (user suggestion, same day) — kind is data,
worth a silhouette. Still scoped out: position
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
  merged — a refactor text cannot express.
- Nothing needs to live under anything; the graph allows floating
  nodes. Orphans are the pool: autocomplete searches it alongside
  module-scoped definitions, and a pool browser lists and manages it.
- Garbage collection is explicit only.
