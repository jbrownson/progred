# Grap and Projections

Decision record, 2026-08-08. This supersedes the 2026-07-25 decision
to make Rust-to-wasm plugins the first projection language. The wasm
spike remains in the tree as proven infrastructure, but it is no
longer on the application's live f64 projection path.

## The Decision

Bootstrap Progred with Grap, a small language embedded directly in the
existing GID data. Grap-defined functions are strict; effects enter
through registered Rust functions and explicitly scoped capabilities.
Rust functions receive raw operands and control recursive evaluation.
Grap is not another syntax tree and adds
nothing to `Value`: records, lists, blobs, and cell references remain
the whole data model. A fixed library gives a few cell identities meaning,
and the evaluator interprets records using those identities. Numeric,
geometry, and future CAD meanings are separate Grap libraries, not
language primitives.

This changes the near-term aim. We are building an Inventing on
Principle CAD/CAM system, not trying to host arbitrary existing
languages first. A small language whose program is already the
semantics graph lets projections and evaluation evolve together,
without a text/compiler round trip in the interaction loop. Existing
languages may still become projections over resolved semantics graphs
later.

## Projection and Display Layers

Normal display is an ordered chain of partial projections over `Value`,
ending in a total structural projection which can show any GID value. The
structural projection recursively re-enters the same dispatcher for every
child instead of owning a closed set of leaf cases. `projection` is the
runner: location lookup and trying an explicit list of partials. Each
partial is a function that checks its own preconditions. Each built-in
library module exports one `Library` value containing its cells, foreign
functions, and ordered partial projections; `stack::load` folds those
values and builds one reusable
projection — text, f64, and the `evaluate` value partial — above the structural fallback. Raw is that
fallback alone. This composed projection is passed explicitly through
recursion; it is not hidden in display context.
`descend` receives the parent `Value` and an ordinary GID `Step`, extends
stored source provenance, and resolves that location. It may prepend
contextual partials to the current projection for a present child and may
supply a concrete layout for a missing child. Omitting either specialization
uses the current projection or the ordinary pending state respectively. A
missing layout receives the real missing location but no fabricated `Value`.
A contextual `at` may prepend another composition of partials for one
subtree while retaining its real source path; the current projection remains
behind it. Grap uses this to keep code-shaped descendants in the Grap
projection without losing text, f64, or the total fallback.
A domain projection should specialize only the structure it fully accounts for.
Raw is the same total projection with its partial layers disabled.
The Grap call projection uses the host's read-only cell lookup to inspect a
stored lambda definition and presents existing arguments in its declared
parameter order, followed by extra fields in the ordinary stable order. This
is deliberately not evaluation: inline and stored lambdas supply useful
source metadata, while computed callables and foreign functions fall back to
the ordinary field order. It supplies that order and its contextual field
projection to the display layer's generic record combinator; the combinator
alone owns delimiters, separators, responsive flat/column layout, and the
choice to break an individual field between its label and value.

Computed values do not use another projection operation. They start the same
projection at a transient root with fresh source provenance. Stored provenance
provides a document path and therefore potential write capability; transient
provenance has no editable document location and may attribute interaction to
the stored expression which produced it. Source/editability and
projection choice remain separate inputs.

A GID projection function has one consistent interface:

```text
projection({value: source}) -> projected-value
```

The callable may be Grap-defined or registered in Rust. Its result re-enters
the same normal projection at a transient, read-only root; returning an
absent-classified value declines and leaves the stored source under the normal
editable projection. Raw never invokes the function. Document-declared panes
use this interface rather than defining a second presentation protocol. The
built-in `drawing` function is one implementation: it wraps its argument in
the ordinary drawing data form consumed by the layout library.
When that leaf renders, its scoped drawing functions send fills directly to
the caller's canvas. `path`, `move to`, `line to`, and `close` mutate only the
path builder in that synchronous scope; a `fill` without an explicit shape
fills the current path. Paths therefore need not become temporary GID
lists merely to cross back into the host.

A partial returns a `Layout<World, Hover>`: boxes, paint-parametric Puri
leaves (`Text` and `Drawing`), the stock host `LineEdit` control, generic hover
claims, and event wrappers. A generic event wrapper holds a Grap
callable; realize turns platform events into GID records and invokes
the callable with capabilities closed over the wrapper's projection
site. The path stays in Rust. Site annotations and the current
selection are get/set capabilities in that temporary overlay, and
their writes commit only when the handler returns a non-absent result
without diagnostics. Text and f64 request the stock Rust line control
with their spelling, affixes, and Grap write-back rule. Progred lowers
that control through Puri. Puri itself is the leaf language: text plus
fill/stroke/clip canvas programs over its ordinary shape vocabulary, with no
editor metadata on either. The layout language owns grouping (`at`, `descend`,
`alternatives`, `surround`, `hug`). The live interpreter measures that layout;
callbacks become Puri handlers. There is no projection-action enum or
central reducer: a callback receives `&mut World` when it fires. The
structural walk is the total fallback and owns GID paths, editing,
and source interaction. Focus and cursor live on the selection.
Line breaking remains in the layout layer for now rather than adding
HTML-like flow or `<br>` semantics prematurely.

The earlier Grap design was rejected for good reasons, but they were
properties of that design rather than of an embedded language:

- Parameters were inferred from free cells. The concrete bootstrap uses
  an explicit ordered list of parameter cells and call fields labelled
  by those cells. This representation is deliberately provisional while
  pattern matching is worked out through editable examples.
- Cell resolution changed meaning based on whether the cell happened
  to have a stored value. It is now ordinary lexical lookup followed
  by document/library lookup.
- Templates, general macros, hygiene, and mint-on-instantiation were
  being designed before a useful evaluator existed. The bootstrap
  instead gives Rust implementations raw operands and the evaluator's
  calling environment. Returning a raw operand produces data; recursive
  evaluation is explicit.
- Grap was being weighed as a general replacement for existing
  languages. Its current job is smaller: make the system immediately
  live, then provide the substrate from which richer projections can
  be built.

## Representation

Semantic labels are library cell IDs, not strings. The IDs are
once-minted random 128-bit cell identities, checked in as library
facts; they are not derived from, or hashes of, their names. Their
simple names are ordinary GID facts supplied by the `name`
library, not metadata in the cell table.

Core Grap source forms use `function`, `params`, and `body` to
distinguish applications and lambdas from ordinary records. Evaluation
uses three more identities in explicit callable values: `closure`,
`environment`, and `ffi`. The core library also names the Rust-implemented
`evaluate` function and its `expression` argument; `evaluate` is a normal
registered call rather than evaluator syntax. Core Grap has no number or
geometry type and no arithmetic or geometry operation.

The built-in Grap library offers an `evaluate` value partial alongside the
runtime evaluator. Its separate `grap` field is ordinary domain vocabulary:
it says its contents are Grap source but requests no evaluation and currently
changes no editor behavior. The library also projects calls and lambdas as
code. The structural default follows cells into their definitions. Grap
expression and callable subtrees prepend a shallow named-cell projection, so
bindings are compact references at their use sites. Nested declaration and
inert-data positions prepend the deep cell form again: parameters, binders,
and patterns expose editable definitions, while quote exposes its template as
data. These contextual projections can alternate as constructs nest. FFI
values use the same reference presentation, because
foreignness is not caller syntax. Argument labels retain the normal
named-first alphabetical order.
The control library composes ahead of that general call projection: a
well-formed `match` call displays its subject followed by ordered
`pattern → expression` cases; binding patterns remain
visibly distinct as `bind name`. Malformed match-shaped data declines this
projection whole and falls through to the ordinary call or structural view.
The synthesized arrow targets the case's expression, like a field head
targets its value; the whole case remains a structural navigation landmark.
A record with an `evaluate` field is replaced by the stored
expression (nested under that field so editing stays on
`…+Key(evaluate)`), then `→`, then the returned `Value` recursively
projected from a transient, read-only root. If that result has a
`evaluate` field, the same partial matches and evaluates it, continuing
the same fuel allowance. Recognition
is open: other fields do not block it. The default
projection therefore shows `expression → result`, while Raw shows
the stored record. The `→` is a dim leaf, not graph
data. The field is not an evaluator form: the evaluator does not observe the
field, and a host that never loads the projection never sees it.

A lambda is a record requiring two semantic fields:

```text
{
  params: [x, y],
  body: ...,
}
```

The parameter values are cells. The list establishes order, and each
cell is lexically bound to the value under the matching call field while
evaluating the body. Parameter identities need not be unique, and
`function` itself may be a parameter: both simply reuse that application
field under the ordinary call rules. The lambda is an open record pattern:
unrelated fields do not stop the record from being a lambda. A lambda may
be inline and anonymous or be the value of a cell; naming and recursive
reference need no additional language identity.

Evaluating a lambda produces an explicit closure value:

```text
{
  closure: {
    params: [x, y],
    body: ...,
    environment: ...,
  },
}
```

The environment is an ordinary record containing the current lexical
bindings. A returned closure is therefore already a projectable Grap
value rather than an opaque host object. Calling it extends that captured
record with its evaluated arguments before evaluating the body. The
wrapper positively identifies the evaluated form, so an untagged
`{params, body}` value always remains a lambda.

A call is a record with a `function` field and fields labelled by the
function's parameter cells:

```text
{
  function: sum,
  x: ...,
  y: ...,
}
```

The apparent names above are binder sugar in the GID text bridge. Matching is
by cell identity. Renaming a parameter changes no program reference,
and there is no parallel symbol-ID system. Additional top-level call
fields are valid GID data and do not prevent the selected function
from being called.
The result arm under `evaluate` is transient and read-only; the expression
arm remains an ordinary visible projection at the `evaluate` field path.
Raw exposes only the stored wrapper and any such metadata.

Numbers remain a library convention rather than a data-model variant.
The separate f64 library represents an f64 as eight little-endian bytes
under its `f64` label. Recognition is positive and open: the presence
of a valid `f64` field establishes the numeric facet even if the record
also carries provenance, history, or some other facet. It defines
strict binary `add` and `multiply` clauses with `left` and `right`
parameter fields, and registers their implementations in Rust. The
geometry library applies the same rule to `circle` and `radius`; its
Rust-backed circle constructor consumes the f64 library's
representation. Neither library changes Grap or `Value`.

The Rust evaluator may lower a recognized f64 facet to an unboxed host
number while an evaluation is running. The f64 library registers the
open decoder and canonical encoder; Grap syntax does not recognize a
numeric form. A lowered source number retains its complete original
`Value`, so passing `{f64: bits, metadata: value}` through a binding
returns that exact enriched value. A computed number is encoded back to
ordinary GID only when a generic operation or the evaluation result asks
for a `Value`. Environments carry these runtime values directly, and
optimized Rust functions may accept and return them; ordinary foreign
functions continue to receive and return GID values through adapters.
Records and lists constructed during evaluation likewise retain runtime
children. `quote`, destructuring, and the list iteration functions can
therefore route an unquoted number through nested containers without first
encoding it as GID and decoding it again. Materialization remains recursive
and exact at the public result boundary.

## Evaluation

Ordinary projection does not implicitly run call-shaped records. It
can therefore show a lambda or expression as editable
structure in one part of a document while an `evaluate` field elsewhere
references that same cell and shows both that ordinary cell projection
and its result. Because the expression arm is ordinary, hovering it can
highlight the cell's other projections. The returned value goes through
the same text, number, geometry, cell, list, and record projections as
stored data, including `evaluate` if the result carries that field.
Transient children are
currently read-only and map
selection back to the stored wrapper rather than pretending
to have document paths.

Evaluating a cell is transparent:

1. A lexical binding with that cell identity wins.
2. A cell registered by a library as a foreign function evaluates to
   `{ffi: cell}` without document resolution. When that value reaches
   function position, the registry supplies its parameter shape and host
   implementation; the GID value does not duplicate either.
3. Otherwise the cell is resolved through the caller's document-over-
   library source and its value is evaluated.

This is one dependency made honest, not a special reference/value
mode stored on cells. A parameter may have a name or even a document
value; within its function body the lexical binding wins because that
identity is the parameter.

Blobs, lists, and unrecognized records are inert data. The evaluator
does not search them recursively for expressions. A record containing
the fixed `function` label is a call, and a record containing both
`params` and `body` is treated as a lambda. The selected
implementation drives recursion. Grap-defined functions evaluate each
declared argument before evaluating their body. Rust implementations
instead receive raw argument expressions plus one Rust calling
environment; f64 multiply chooses to evaluate both of its operands in
that environment. An unrelated record evaluates to itself without
inspecting its children.

Grap-defined functions are strict call-by-value, not eager traversal of
all GID data. Rust implementations are evaluator-aware: an `if`
implementation can evaluate its condition and exactly one raw branch,
while a matcher can evaluate a selected branch in an extended copy of
the calling environment. Rust arithmetic uses the same interface but
immediately evaluates every operand. This keeps one surface call shape
without adding strict/raw modes to Grap parameters.

The control library supplies one such operation, `match`:

```text
{
  function: match,
  value: subject,
  cases: [
    {pattern: pattern1, expression: expression1},
    {pattern: pattern2, expression: expression2},
  ],
}
```

`match` evaluates its subject exactly once, then tries the cases in
list order. Record patterns are open, list patterns are exact and
ordered, and blobs and bare cell references match literally. A pattern
record containing `{bind: binder-cell}` captures the corresponding
subject value; another occurrence of that binder must capture an equal
value. The first matching case extends the calling environment
with its captures and evaluates its expression. If none match, `match`
returns an ordinary absent without a diagnostic. A final binder pattern
is the uniform catch-all when one is wanted. An absent returned by the
selected expression is still its result and does not fall through to
another case.
`match` destructures the evaluator's runtime values directly; it does not
create a hidden graph binding or make patterns depend on an enclosing match.

There is no evaluator-level quote or literal form. At the Rust boundary
an operand is already an inert expression; returning its expression
returns data because call results are not evaluated again. The control
library uses that property to implement `quote` as an ordinary registered
Rust function:

```text
{
  function: quote,
  expression: {
    preserved: {function: some-call},
    interpolated: {unquote: expression},
  },
}
```

`quote` walks the original raw expression structurally. Cells and blobs
are copied, and list positions and record labels are preserved. A record
containing `unquote` is a replacement leaf: its value is evaluated in
the quote call's environment and the result is spliced into the output.
That result is not walked again, which gives one layer of interpolation.
An unquote-shaped record outside `quote` is ordinary self-evaluating data;
the core evaluator never recognizes it. Ordinary data needs no quote to
evaluate to itself—quote is useful for preserving recognized expression
forms as data and for explicit interpolation while constructing data.
First-class suspended work can still pair an expression with its
environment as ordinary GID data when a program genuinely needs to
store or forward that pair.

These conventions match what is present, not what is absent. Record
patterns are open unless a particular domain explicitly says
otherwise. Text and f64 line projections use the same open
recognition as Grap, so a later projection can wrap them (a unit
around a number). Each line's update sees the current value and the
new text, so it can keep extra fields or replace the value.

Every external cell read is collected as a dependency. The set is
reported even when evaluation produces an absent, ready for future
precise invalidation. Every evaluation also has explicit fuel, and cell
cycles produce a stable absent. Invalid Grap never damages the
underlying document: its absent is projected like any other normal
form, and the stored expression remains editable in Raw or wherever
the same expression cell is projected outside an `evaluate` field.

The evaluator runtime lives in its own `grap` crate and depends only on
GID plus its persistent-map implementation. It knows the tagged
`{absent: reason-cell}` result convention and its own stable reasons. It
also provides implementation-level runtime records, lists, and an f64
carrier, but the f64 library supplies the GID recognition and encoding
functions; the evaluator assigns the carrier no surface syntax or numeric
semantics.
It knows no names, projection, geometry, UI, file, or Linebender concepts.
The `progred-libraries` package contains one module per
built-in conceptual library: Grap, name, text, absent, control,
f64, and geometry. Each module exports its complete `Library` value.
`Library` is the product of the cells, foreign-function table, and
ordered-partial-list monoids. The editor folds those values once into
its loaded `Stack`; a later foreign table overrides a shared cell.

A registered Rust implementation receives the call record, the calling
environment, and the live evaluation context. It looks up the fields it
consumes as raw argument expressions and may recursively evaluate any
of them through that context. Its semantic result is still an ordinary
`Value`, although an optimized implementation may retain an equivalent
`RuntimeValue` until the host boundary; the host `Result` only propagates
evaluator halting
such as exhausted fuel. Rust environments remain validated evaluator
values and become GID records only through an explicit conversion.
The registered `evaluate` implementation evaluates its environment
argument, converts the resulting record to an environment, then asks the
same evaluator to interpret its raw expression argument there. It is an
ordinary foreign call, not another form recognized by `eval`.

Grap-defined functions deliberately have less direct host authority:
their arguments are evaluated before binding, while effects are calls
to explicit FFI values and scoped capabilities. Rust currently owns
evaluation-control operations such as conditionals and matching. A
separate Grap-defined macro representation can be added later if a
concrete need justifies it; every Grap function does not need to become
an operative in advance.

Every evaluation returns a `Value`, including malformed programs,
missing cells, cycles, and exhausted fuel. An absence is the open tagged
value `{absent: reason-cell}`. Host-facing diagnostics accompany core
absences with occurrence-specific detail,
such as which cell was missing, without introducing a separate host
result channel into Grap or changing Grap control flow. When an
evaluated call returns an absence, the result retains both the explicit
tag and the stable reason identity.

The bootstrap f64 and geometry libraries define stable library cells
for their absent reasons and return tagged values containing those
identities: several semantically distinct custom nulls, not names or
freshly allocated reason identities. Each reason cell may contain names,
documentation, translations, or other static metadata. Recognition and
control flow inspect the tag and CellId payload directly, never those
human-facing facts.

## First Vertical Slice

For a record with an `evaluate` field, the default projection shows
the stored expression, an arrow, and its recursively projected
result—an f64 as text, and arbitrary GID data structurally.
[`examples/grap-demo.gid`](../examples/grap-demo.gid) is
the focused interactive playground: three editable f64 cells feed
direct foreign calls, nested calls, the registered `evaluate` function
with an explicit empty environment, Grap-defined functions, a circle,
a `match` which destructures that circle and binds its radius, and a
function which uses quote/unquote to generate cases for another
match; it also keeps extra call metadata in Raw, demonstrates inert
returned data, and shows stable type,
missing-argument, and not-callable absents as ordinary projected
results. The demo projects one Grap expression cell both directly and
by reference under `evaluate`, making their shared identity visible through
hover while the latter also carries its computed result.

The broader checked-in [`examples/sample.gid`](../examples/sample.gid) carries the same evaluation path
inside the raw editor's structural examples:

- `pitch` is a cell containing f64 `2.5`.
- `double` is a Grap function with an explicit `amount` parameter. Its
  body calls the f64 library's Rust-backed `multiply` with `amount` and
  f64 `2` under the `left` and `right` fields.
- the roof contains a call to `double`, passing `pitch`; its projected
  result is `5`.
- a nested expression multiplies that result by `8`, passes the result
  as the radius of `circle`, and projects the resulting radius-40
  circle as ordinary GID structure.

The `evaluate` field belongs to projection rather than evaluation. The
default projection replaces such a record with `expression → result`;
Raw projects the stored record.
Compact f64 source values edit as decimal text while continuing to store
the f64 library's byte representation, so changing `pitch` immediately
changes both the `double_pitch` result and the projected circle record. This is
intentionally not yet the CAD interaction: it provides a tangible GID
edit, evaluation, and projection loop from which the evaluator can be
redesigned.

## Near-Term Direction

The next useful growth is driven by one interactive geometric
construction, not by filling out a language checklist:

- introduce the smallest geometry values and foreign operations the
  construction needs;
- project evaluated geometry through Puri rather than only text;
- make a direct manipulation write its controlling GID values;
- use the dependency set to reevaluate only affected results if full
  frame evaluation becomes material;
- add absents and evaluation traces as projections over the same GID value,
  while keeping Raw as the escape hatch.

The next language work should be forced by manipulating this example:
use `match` when the construction needs conditional structure, refine
patterns from concrete editing experience, and make a thunk or cell
evaluation projection only when the interaction needs one.
Grap-defined macros and general code generation remain out of scope
until a concrete transformation requires them.

## Parked language notes

Working notes, not current work. Take them one at a time, and only
when a construction or editing problem forces the change.

- `quote` exists primarily for `unquote`: walk structure, splice, leave
  the rest unevaluated. A sibling `literal` that copies the same way
  but treats `{unquote: …}` as data would complete the pair. Both are
  ordinary Rust library functions, not evaluator forms.
- An absent from a selected `match` case is that case's result, not
  fallthrough. Matching *on* an absent subject is separate and already
  works (the subject is an ordinary value, often one of the stable
  absent cells). Absents stay values, not implicit failure.
- `params` is a list of cells. A user function may reuse library or
  other binders (`left`, `right`, another function's parameter).
  Minting a fresh cell per parameter is an authoring default, not a
  language rule. Shared vocabulary cells *are* a calling convention;
  `add` and `multiply` already share `left` and `right`.
- Patterned function arguments can use the same destructure as `match`.
  A call is already an open record, so a pattern `{left: {bind: x},
  right: {bind: y}}` matches `{function: f, left: a, right: b, …}` and
  ignores `function` and extras. That would replace the ordered
  param-list binding story. `match` remains as local match (same
  matcher, or an immediately applied patterned lambda).
- Secondary marks should follow cells only. Lighting up equal blobs,
  lists, or compact text is leftover from models where those were
  identity-graph nodes. Selecting a cell should still mark every
  mention of that cell.

## The Superseded Wasm Spike

The 2026-07-25 spike proved that Progred can compile Rust to wasm over
pipes and safely host pure projection plugins in Wasmtime. ABI 1 uses
three exports (`abi_version`, `alloc`, and `project`) plus linear
memory. Calls receive no imports, use a fresh Store, and are bounded by
epoch interruption. The compiler service invokes an explicitly chosen
rustup toolchain in a per-call temporary directory and returns
structured rustc diagnostics.

That code, the f64 guest, and their tests are retained together in the
`experiments/rust-wasm-projection` crate. They are useful evidence for a
future foreign-language boundary and remain covered by the workspace
tests without appearing to be part of the active architecture. The app
contains neither the compiler and host nor a Wasmtime dependency, and
the active projection does not load the guest.
