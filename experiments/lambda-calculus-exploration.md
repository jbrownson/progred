# The Lambda Calculus Exploration

2026-07-14 through 2026-07-20, in the ~/git/lambda-editor sessions.
This document records the line of thought — the questions, the dead
ends, the reversals — not just the conclusions. The conclusions landed
in `prototype-linebender/docs/model.md` ("Values and Cells" design
brief); the verified research map with ~150 link-checked sources lives
at https://claude.ai/code/artifact/fa451f1a-9379-4e5b-9e73-b2ba9dae330c.

Written for the future reader (probably us) who wants to know WHY the
data model looks the way it does, and which attractive roads were
walked far enough to know they end.

## I. The crisis (July 14)

Adding lists to the gid data model felt arbitrary. The feeling
generalized: every data-model decision seemed like a whim among
infinitely many, and lambda calculus — one substrate, three
constructs, everything else defined within — looked like the way out.
The brain dump had six ideas: (1) a direct-manipulation lambda editor
(clambda already existed and was further along than remembered);
(2) bootstrapped storage — the editor's own format lambda-encoded,
frames as projections of LC-encoded scenes plus an event function;
(3) MicroHs-style combinator execution; (4) projections recognizing
encodings (Church numerals rendered as numbers) so "adding lists"
becomes a definition, not a data-model decision; (5) compiling by
modeling wasm inside LC and extracting instruction sequences;
(6) AlphaZero-style search over lambda terms for optimal programs.

The underlying question, stated later and better: WHY ISN'T EVERYTHING
LAMBDA CALCULUS?

## II. The research verdict (July 14)

An eight-thread verified research pass mapped all six ideas onto
existing work. The compressed scorecard:

- (1) genuine gap — no polished direct-manipulation editor for
  untyped LC with a graphical surface exists; clambda's interaction
  model already exceeded the closest prior art (Visual Lambda,
  LambdaJS). Eros (Elliott 2007) the closest single-paper ancestor.
- (2) exists at toy scale, solo-built: bruijn and Lambda Screen
  (Marvin Borner) are the substrate half; the interactive
  self-hosting combination is a research program, not a feature.
- (3) solved, off the shelf (MicroHs; Kiselyov translation; Ben
  Lynn's series).
- (4) hard core, workable rim — see the crux below.
- (5) direction error + already done rigorously (CertiCoq-Wasm).
  A machine modeled in LC is an INTERPRETER — the spec, not the
  compiler. Compilation is a directed transformation pipeline.
- (6) search half occupied and superseded (AlphaDev → AlphaEvolve;
  LLM-guided beats bespoke RL); abstraction-discovery half validated
  and cheap (DreamCoder proved map/fold/filter EMERGE from
  compressing lambda corpora; Stitch does it in milliseconds, in
  Rust).

The crux, four facts that killed the strong version of the plan:

1. Recognition is heuristic forever. Scott–Curry: whether a term is
   β-equivalent to a Church list is undecidable. Workable pipeline:
   fuel-limited reduction to WHNF, first-order syntactic match, grey
   box on timeout (Lambda Screen's trick). And normal forms are
   ambiguous — λf.λx.x is Church 0, false, and nil SIMULTANEOUSLY.
   The information that disambiguates an encoding is a type.
2. Terms can't hold what an editor needs. No names, layout,
   provenance, stable identity in terms-modulo-α. Unison keeps all
   of that in an engineered codebase layer OUTSIDE the terms. THE
   ARBITRARINESS RELOCATES; IT DOES NOT DISAPPEAR.
3. The performance wall is documented, not speculative (Church
   pattern-match O(n) per match; Dhall's own docs; lambda-8cc's
   240 GB fizzbuzz).
4. The cure for "lists felt arbitrary" is INITIALITY, not
   untypedness. A list is the initial algebra of F(X) = 1 + A×X —
   uniquely determined, zero design freedom. The Böhm–Berarducci
   encoding IS that universal property internalized as a type — and
   it only behaves (no junk) under parametricity, a theorem about
   TYPED terms. The elegance we were chasing is a property of
   System F, not of untyped LC.

The graves, unanimous in direction: Lisp grew vectors/records/hash
tables; Morte→Dhall retreated to native Naturals/Lists (receipts:
dhall-lang #602 — Church-style Natural/lt 5000 5000 exhausted 16 GB;
#125 — the marshalling wall; #3 — no native recursion); Awelon
(clambda's exact shape: minimal calculus + projectional editor +
editable views + acceleration, ~10 years solo, abandoned 2019);
Darklang's structural editor shipped and was killed; VPRI STEPS got
small via a tower of DSLs, not one calculus; Formality→Kind grew
types back. Every system that started at "minimal calculus all the
way down" survived only by adding a second layer — and the second
layer was most of the system.

Decision: keep progred; clambda a probe, not a foundation.

## III. Follow-ups, discoveries, and the corrections ledger

The verification passes surfaced things the first pass missed, and
caught real errors in our own claims. Both belong in the record.

Discoveries: David Barbour's ACTIVE 2026 project glam is "pure,
untyped lambda calculus for metaprogramming of assembly," migrating
to interaction nets — the person with the decade in this design
space converged on the same substrate. Marvin Borner and Barbour are
the two most valuable people to talk to. Apparently-unclaimed niches
found and filed: Stitch-learned combinator bases (Augustsson
hand-did exactly this for MicroHs's Fig. 3 basis — "adding K5 makes
it slower" is the compression≠speed warning); vanilla-egg equality
saturation over binder-free combinator graphs; a "SERV of SKI"
(bit-serial/width-parameterized reduction core — nothing exists);
succinct-data-structure (rank/select, PDEP/PEXT) navigation of BLC
terms for the projection/readback side.

Corrections we had to make to our own claims along the way (the
method working as intended): dhall #125 is the marshalling wall, not
the performance wall (#602 is); GraalVM Truffle is the FIRST
Futamura projection in production, not the second; the BLC
self-interpreter is 206–210 bits, not 230; Lean's production kernel
TRUSTS the Nat-jet name correspondence (the defining-equation checks
are Lean4Lean's, the external verified checker); the "Scott-encoded
number costs ~200× the bits" claim was the heap-pointer
representation, not the encoding — BLC-serialized it's ~17×, as a
tuple ~9×, and the ladder's limit (payload + context reference, ~1×)
IS the jet representation, which turned the objection into the
thesis.

## IV. The hardware descent (why is LC slow — really?)

"Why isn't everything lambda calculus" decomposed into layers with
different answers:

1. Encoding density — Church numerals are unary; a CHOICE, fixed by
   binary encodings. Not LC's fault.
2. Racing a dedicated parallel circuit — a 64-bit hardware add is a
   spatial carry-lookahead circuit; ~400× is lost just by
   serializing it. An ALU is a hardware jet for fixed-width binary
   arithmetic.
3. Bits living in the heap — boxing, pointer-chasing, closures:
   the graph-reduction constant factor.
4. Genuinely asymptotic residue — small: β-counts are a reasonable
   cost model (Accattoli–Dal Lago); purity costs at most log-ish
   factors, sometimes zero (Pippenger vs the lazy rescue).

SPJ's verdict on the 1980s hardware wave ("we were building an
interpreter in hardware... build a compiler" — Peterman Pod, June
2026) drew our best pushback: a modern CPU IS an interpreter (~70 pJ
of instruction machinery around a 0.1 pJ add — 99% self-
interpretation), so the argument begs the question as stated. The
non-circular core is BINDING TIMES: don't re-derive program-invariant
facts at run time, on any substrate. TIGRE (software, better staging)
beating NORMA (custom hardware, worse staging) convicts the 80s
machines without mentioning Intel. Reduceron didn't refute SPJ; it
obeyed him — staged templates, hardware only for the dynamic residue.

The jets thread converged from both ends: jets and intrinsics meet at
the fixed point (reference definition + native twin + boundary
materialization); the difference that remains is WHICH SIDE IS
NORMATIVE. The trust spectrum, all shipped: Nock (declared hints,
convention-checked, jet-mismatch bugs as the cautionary record),
Lean (kernel jets keyed by hard-coded NAME, trusted; piggybacking as
the extension mechanism — UInt32 → BitVec → Fin → Nat inherits the
Nat jets through definitional unfolding), Coq (axiomatized
primitives), Simplicity (per-jet Coq proofs), Agda (the cleanest
statement: "both an Agda definition and a primitive implementation").
The projectional editor's advantage over all of them: dispatch by
BINDING IDENTITY at edit time — earlier and more robust than hints,
names, or hashes.

The width dial: word width is the spatialization knob (a 64-bit ALU
is 64 one-bit ALUs unrolled in space); folding/unfolding are the
formal transformations; SERV→QERV shipped the dial as one RTL
parameter (June 2026: 1-bit↔4-bit, 3–4× at +20% area); Stripes/Bit
Fusion built its temporal and spatial duals in ML silicon. A
bit-serial adder consuming LSB-first streams IS add on lazy bit
lists — the hardware shadow of the lambda encoding (and the fossil
reason x86 is little-endian). Linearity = streamability =
spatializability kept appearing as one property.

The synthesis that closed the original question: EVERYTHING IS
TRANSISTORS. The transistor (better: the restoring switch, whose
feedback loop — the latch — is the fixpoint in space) is the
best-found mapping from physics to computation; you can't move or
mint them during a computation; processors are the multiplexing
illusion that lets us run dishonest-but-bounded models of unbounded
constructs. The whole game, at every level, is AMORTIZING
INTERPRETATION over larger recognized units — registers, ALUs, jets,
fusion, carry-lookahead: the same move at different scales — and
units can only be enlarged where structure is predictable ("static
routability," why arithmetic gets silicon and general beta doesn't).

## V. The type-system detour (dead end, instructive)

Proposal seriously entertained: model the machine in the types —
"Nat is a fantasy; put parameterized ℤ/n at the bottom." Killed by
three arguments worth keeping: (a) "parameterized ℤ/n" is
parameterized over n ∈ ℕ — the naturals reappear as the index of the
family that was to replace them; (b) ℕ = List Unit — any system with
inductive data (terms, lists, bitstreams) already contains ℕ;
rejecting it while keeping lists is incoherent; (c) honest labels:
ℤ/n doesn't lie — writing ℕ when you mean ℤ/n lies, and C ran that
experiment ("int") at civilizational cost; conversely ℕ-as-limb-list
is honest partiality (fails by exhaustion, never reinterprets).
What survived: machine types belong in the vocabulary as DESCRIBED
OBJECTS with truthful semantics (Sail's RISC-V model now exists in
Lean); the invariant objects belong at the bottom because they're
what doesn't change when hardware does. "Bounded resources,
unbounded types; finite object language, counting metalanguage."

## VI. The data-model endgame (numbers, atoms, identity)

The number question ran through four positions, each killed by the
last: (1) GMP-style bignum atom — fine, but then (2) "just
MPFR/one number type" collapsed under the one-spelling law
(canonicalizing across precisions yields the dyadic rationals, which
can't represent 0.1 — the commonest typed literal); (3) decimal
m·10^e (a pair of mpz's — BigDecimal/libmpdec) fixed literals and
subsumed integers (e ≥ 0 as a shape); then (4) "just store strings —
languages encode numbers as strings in source" nearly won, and its
refutation produced the real principle: text languages recover
number-ness from a LEXER at parse time; a projectional substrate has
no parse time, so the kind must travel with the datum — the atom
kind is the smallest type system, and it's load-bearing for
identity ("1" vs "1.0"), ordering ("10" < "2" as strings), and
interop (JSON's 5 vs "5"). Resolution: numbers fail the admission
test — NOT atoms; a projection convention over canonical spellings.
(The convention half was pulled back at implementation, 2026-07-20:
numbers left the model with nothing replacing them — encodings
arrive later as libraries; see model.md's v3 correction.)
The admission criteria themselves crystallized late: an atom earns
admission when it maps to the machine world, matters to users,
CANNOT be efficiently encoded by the other constructs, or is needed
to bootstrap a zero-convention editor. Strings pass (bootstrap),
blobs pass (opaque payload, hex floor — the old "codepages"
rejection answered: identity stays bit-decidable, interpretation is
projection-level), numbers fail, f64 parks, rationals out, reals
excluded by the doc's own decidable-normal-form law (constructive
reals — Boehm's Android-calculator library — are computation-layer
codata; π is a function from precision to approximation).

Identity: the v2 asymmetry (maps are entities, lists are values) was
the "arbitrary asymmetry" the JSON-rejection paragraph had already
named. The factorization — all shapes structural, identity a CELL
holding one value — is the Clojure/Datomic lineage (values +
identities; egal equality), and it dissolved a stack of warts (kind
stickiness, conversion, wrap) while making inline value-records
expressible. One correction en route: "mint a cell when you need to
point at something" overclaimed — a diagnostic pointing at line 3
must not edit the document, and minting requires addressing first.
PATHS ARE PRIMITIVE; IDENTITY IS A DURABILITY UPGRADE. In-data
references get cells; about-data references (selection, diagnostics)
are paths, rewritten or recomputed. A bonus that fell out: inline
definitions project inline BECAUSE they're not references —
value-vs-reference is the projection signal (clambda's model,
rediscovered from the data side).

Neighbors toured and declined: Falcor (path-identity, the fragility
adopted as architecture; dormant), IPLD (content addressing ≠
mutable identity), JSON-LD/RDF (relitigated once, enough), EDN
(steal tagged literals), Datomic (the datom = our triple + time;
entity ids are PARTITIONED SEQUENTIAL LONGS for index locality —
squuids the lesson if our random uuids ever meet a sorted index;
VAET = backlinks for free; cardinality-many is a SET — they declined
the ordered-list fight we won with fractional positions). For
multiplayer someday: Figma's model (per-property LWW under server
order, fractional child indexes) is nearly isomorphic to ours; read
Evan Wallace's post before designing anything.

## VII. Platform and language questions (considered, settled)

Clojure: seriously considered after the Hickey convergence
(HumbleUI/Skija the philosophically-matching stack; Membrane = Puri's
design brief independently; the REPL-against-a-live-document the real
draw). Declined — high-risk diversion, Rust ecosystem preferred; the
document world already follows the Hickey philosophy (spec-stance
shapes) while the implementation stays typed. Racket: read the
papers, skip the platform — with one souvenir: clambda's
identity-based structural binding gives HYGIENE BY CONSTRUCTION,
dissolving the problem scope-sets spent decades on. Lean: bumped high
— the shipped registry pattern, ProofWidgets as projections prior
art, Syntax/Expr entry points make progred-as-Lean-frontend natural;
embed later, JS first ("normal"). grap — the projectional
graph-lisp, where homoiconicity's power survives and its syntax cost
vanishes — named and parked (nb: the name collides with
Bentley/Kernighan's troff grap). Calcit/Cirru noted as the decade-
long solo witness that no-text-source living works, now pitching
tree-editing as AI-agent-friendly — an argument worth quoting.

## VIII. What survived — the principles

1. Admission by stated criteria, not vibes (the four tests).
2. Honest labels: a type names the object it actually is; partiality
   honest, silent reinterpretation never.
3. Interpretation lives in context; density and speed come from
   moving it there ("a type system is a compression dictionary for
   values; a jet registry is its runtime codec").
4. The registry: reference definition (normative) + accelerated twin
   + projection, keyed by binding identity — jets and projections
   are the same construct pointed at CPU and screen.
5. The arbitrariness relocates; put it where it's semantically
   invisible (runtime), never in what documents can say.
6. Paths primitive, identity a durability upgrade, minting an
   authoring act.
7. Linearity = streamability = spatializability.
8. Bounded resources, unbounded types; exhaustion, not wraparound.

Parked threads, with owners-in-waiting: LIVE 2027 submission
(clambda demo); the TT "SERV of SKI" (TTIHP26b closes Sep 2026); the
Stitch-combinator-basis and egg-over-combinators experiments; writing
to Barbour and Borner; the decimal convention projection; grap.

The opening question, answered in one line each way: everything
isn't lambda calculus because everything is transistors — and lists
were never arbitrary, they were the initial algebra of 1 + A×X
waiting for us to notice.
