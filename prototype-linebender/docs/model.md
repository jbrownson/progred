# Data And Editor Model

Date: 2026-07-03

## Data Layer: gid v1, Unchanged

- IDs are GUID | SID | NID. GUIDs are minted identity for mutable
  nodes; SIDs and NIDs are identity the value itself carries. Edges are
  single-valued per label; labels are IDs.
- Lists are graph structure by convention (cons/empty cells with
  well-known UUIDs), not a data-layer primitive and not values.
- There are no compound values.
- No special fields in the data layer. The editor layer treats some
  labels specially (name, isa, cons) the way a text editor treats
  syntax specially while ASCII stays dumb.

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
  `[2, 2]` both elements are the same NID; only the cell distinguishes
  them. Cons cells manufacture position identity — that is their
  justification, not Lisp nostalgia.
- Coupling identity semantics to shape (records mutable with identity,
  lists immutable values) is an arbitrary asymmetry. One law instead:
  GUIDs are minted identity, scalars carry their own, order is
  structure.
- Content-addressed compound IDs (Unison-style hashing) were also
  considered and rejected: hashing a graph requires declaring which
  edges are content and which are incidental, and arbitrary graphs have
  no natural content boundary. To share a structure, reference its
  node — that is what the model is for.

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

## Selection: Splice

Selection is (location, mode): the node at a path, or the gap between
that node and its parent.

- Gaps subsume list insertion points: between items, before the first,
  after the last (the gap before the terminator), and the sole point in
  an empty list. Whole-list versus before-first-item stops being
  ambiguous.
- Splices anchor on cons-cell GUIDs, so they stay valid across sibling
  edits — the stability that index-based positions cannot offer.
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
