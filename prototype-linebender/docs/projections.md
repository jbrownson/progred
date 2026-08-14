# Grap and Projections

Decision record, 2026-08-08. This supersedes the 2026-07-25 decision
to make Rust-to-wasm plugins the first projection language. The wasm
spike remains in the tree as proven infrastructure, but it is no
longer on the application's live f64 projection path.

## The Decision

Bootstrap Progred with Grap, a small language embedded directly in the
existing graph data. Grap-defined functions are strict and pure, while
registered Rust functions receive raw operands and control recursive
evaluation. Grap is not another syntax tree and adds
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
ending in a total structural projection which can show any graph. The
structural projection recursively re-enters the same dispatcher for every
child instead of owning a closed set of leaf cases. `projection` is the
runner: location lookup and trying an explicit list of partials. Each
partial is a function that checks its own preconditions. Library packs
offer those functions; the walk composes them — currently
`conventions::compact` and the `grap` field — above the structural
fallback. Raw omits the pack functions. This composed projection
is passed explicitly through recursion; it is not hidden in display context.
`descend` receives the parent `Value` and an ordinary graph `Step`, extends
stored source provenance, and invokes the supplied projection on that
unresolved location. The projection performs lookup, which lets its total
fallback project a missing field or element as the ordinary pending state.
A domain projection may replace only the structure it fully accounts for.
Raw is the same total projection with its partial layers disabled.

Computed values do not use another projection operation. They start the same
projection at a transient root with fresh source provenance. Stored provenance
provides a document path and therefore potential write capability; transient
provenance has no editable document location and may attribute interaction to
the stored expression which produced it. A Grap result also uses a composition
with the Grap field projection removed. Thus source/editability, projection
choice, and display interpretation remain three separate inputs.

Compact domain projections target a small display vocabulary: styled text, a
generic line editor, vector graphics, rows, and columns. Its live interpreter
measures those operations into Progred's baseline-box layout and draws the
resulting leaves through Puri's canvas. The total structural fallback still
constructs layouts directly because it owns graph paths, editing, and
the detailed source interaction model. The generic line editor carries a
text-to-`Value` handler, so text and f64 use the same display operation with
different parsers; focus and cursor remain interpreter context while source
provenance remains projection context rather than display data. The vector operation carries ordinary
shapes and drawing commands, so a circle is a projection result rather than
a display-language primitive.

This vocabulary is intentionally smaller than the projection system. Grap
evaluation, domain recognition, graph paths, selection, and read-only
provenance are projection concerns, not display constructs. The Rust
vocabulary is expressed as a trait rather than a closed display AST, which
already permits a lightweight test interpreter. A graph-valued display
library can mirror this vocabulary later once graph identities for actions
are concrete. Line breaking remains in the layout layer for now rather than
adding HTML-like flow or `<br>` semantics prematurely.

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
simple names are ordinary graph facts supplied by the `progred-name`
library convention, not metadata in the cell table.

Core Grap source forms use `function`, `params`, and `body` to
distinguish applications and lambdas from ordinary records. Evaluation
uses three more identities in explicit callable values: `closure`,
`environment`, and `ffi`. The core library also names the Rust-implemented
`evaluate` function and its `expression` argument; `evaluate` is a normal
registered call rather than evaluator syntax. Core Grap has no number or
geometry type and no arithmetic or geometry operation.

Progred's projection layer separately defines the `grap` field. The
ordinary record and its `grap` label remain visible, but the projection
for the value under that label shows the stored expression through its
ordinary editable projection, followed by `→` and the returned `Value`
recursively projected from a transient, read-only root. Normal view therefore shows
`{grap: expression → result}`, while Raw shows
`{grap: stored-expression}`. The arrow is projection chrome, not graph
data. `grap` is not a Grap evaluator form, so the evaluator can be used
without Progred and cannot observe the field.

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

The apparent names above are binder sugar in gid notation. Matching is
by cell identity. Renaming a parameter changes no program reference,
and there is no parallel symbol-ID system. Additional top-level call
fields are valid graph data and do not prevent the selected function
from being called.
The result arm under `grap` is transient and read-only; the expression
arm, `grap` field, and rest of its enclosing record remain ordinary
visible projections. Raw exposes only the stored expression and any
such metadata.

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

## Evaluation

Ordinary projection does not implicitly run call-shaped records. It
can therefore show a lambda or expression as editable
structure in one part of a document while a `grap` field elsewhere
references that same cell and shows both that ordinary cell projection
and its result. Because the expression arm is ordinary, hovering it can
highlight the cell's other projections. The returned value goes through
the same text, number, geometry, cell, list, and record projections as
stored data. The projection which produced that subtree is removed from
the explicit composition, so a returned value which itself contains a `grap`
field is data rather than another request to evaluate. Transient children are
currently read-only and map
selection back to the stored `grap` field value rather than pretending
to have document paths.

Evaluating a cell is transparent:

1. A lexical binding with that cell identity wins.
2. A cell registered by a library as a foreign function evaluates to
   `{ffi: cell}` without document resolution. When that value reaches
   function position, the registry supplies its parameter shape and host
   implementation; the graph value does not duplicate either.
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
all graph data. Rust implementations are evaluator-aware: an `if`
implementation can evaluate its condition and exactly one raw branch,
while a matcher can evaluate a selected branch in an extended copy of
the calling environment. Rust arithmetic uses the same interface but
immediately evaluates every operand. This keeps one surface call shape
without adding strict/raw modes to Grap parameters.

The control library supplies one such operation, `case`:

```text
{
  function: case,
  value: subject,
  alternatives: [
    {pattern: pattern1, expression: expression1},
    {pattern: pattern2, expression: expression2},
  ],
  default: default-expression,
}
```

`case` evaluates its subject exactly once, then tries the patterns in
list order. Record patterns are open, list patterns are exact and
ordered, and blobs and bare cell references match literally. A pattern
record containing `{bind: binder-cell}` captures the corresponding
subject value; another occurrence of that binder must capture an equal
value. The first matching alternative extends the calling environment
with its captures and evaluates its expression. If none match, only the
default is evaluated. An absent returned by the selected expression is
still its result and does not fall through to another alternative.
`case` keeps the subject as an ordinary host value while matching; it
does not create a hidden graph binding or make patterns depend on an
enclosing case.

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
environment as ordinary graph data when a program genuinely needs to
store or forward that pair.

These conventions match what is present, not what is absent. Record
patterns are open unless a particular domain explicitly says
otherwise. The current compact f64 and circle views are deliberately
stricter than semantic recognition: they replace a whole record only
when every field in that record belongs to the displayed facet.
Otherwise the structural view remains visible, even though Grap and
the relevant library can still use the recognized facet.

Every external cell read is collected as a dependency. The set is
reported even when evaluation produces an absent, ready for future
precise invalidation. Every evaluation also has explicit fuel, and cell
cycles produce a stable absent. Invalid Grap never damages the
underlying document: its absent is projected like any other normal
form, and the stored expression remains editable in Raw or wherever
the same expression cell is projected outside a `grap` field.

The evaluator lives in its own `grap` crate. It depends on the graph
core and the shared Grap absent and name conventions, but knows no f64,
geometry, UI, file, or Linebender concepts. `grap-f64` and
`grap-geometry` are separate libraries composed by the application.
Each crate returns its graph facts and, when it has Rust
implementations, a foreign-function table. The editor merges those
values; a later table overrides a shared cell.

A registered Rust implementation receives the call record, the calling
environment, and the live evaluation context. It looks up the fields it
consumes as raw argument expressions and may recursively evaluate any
of them through that context. Its semantic result is still an
ordinary `Value`; the host `Result` only propagates evaluator halting
such as exhausted fuel. Rust environments remain validated evaluator
values and become graph records only through an explicit conversion.
The registered `evaluate` implementation evaluates its environment
argument, converts the resulting record to an environment, then asks the
same evaluator to interpret its raw expression argument there. It is an
ordinary foreign call, not another form recognized by `eval`.

Grap-defined functions deliberately have less authority: their
arguments are evaluated before binding and their bodies are pure over
those values and the captured lexical environment. Rust currently owns
evaluation-control operations such as conditionals and matching. A
separate graph-defined macro representation can be added later if a
concrete need justifies it; every Grap function does not need to become
an operative in advance.

Every evaluation returns a `Value`, including malformed programs,
missing cells, cycles, and exhausted fuel. Core evaluator and library
absences are stable absent-cell values. Host-facing diagnostics
accompany core absences with occurrence-specific detail,
such as which cell was missing, without introducing a separate host
result channel into Grap or changing Grap control flow. When an
evaluated call returns one of those cells, the projection shows its
ordinary name as the result.

The bootstrap f64 and geometry libraries define stable library cells
for their absent cases and return those identities as values: several
semantically distinct custom nulls, not freshly allocated occurrences.
Each absent's library value is a record containing `isa: absent`. The
general `isa` relation lives in the independent
`progred-isa` library: it is a convention over graph data, not part of
Grap or `progred-graph`. The absent library owns only the `absent`
classification and uses that relation. Additional static facts can be
added as fields on each absent's record. Absent meaning remains
library data rather than an evaluator feature.

## First Vertical Slice

For a `grap` field, normal projection shows the stored expression, an
arrow, and its recursively projected result—an f64 as text, a circle as native vector
drawing, and arbitrary graph data structurally. The enclosing record
remains ordinary visible data. `grap-demo.gid` is
the focused interactive playground: three editable f64 cells feed
direct foreign calls, nested calls, the registered `evaluate` function
with an explicit empty environment, graph-defined functions, a circle,
a `case` which destructures that circle and binds its radius, and a
function which uses quote/unquote to generate alternatives for another
case; it also keeps extra call metadata in Raw, demonstrates inert
returned data, and shows stable type,
missing-argument, and not-callable absents as ordinary projected
results. The demo projects one graph expression cell both directly and
by reference under `grap`, making their shared identity visible through
hover while the latter also carries its computed result.

The broader checked-in `sample.gid` carries the same evaluation path
inside the raw editor's structural examples:

- `pitch` is a cell containing f64 `2.5`.
- `double` is a Grap function with an explicit `amount` parameter. Its
  body calls the f64 library's Rust-backed `multiply` with `amount` and
  f64 `2` under the `left` and `right` fields.
- the roof contains a call to `double`, passing `pitch`; its projected
  result is `5`.
- a nested expression multiplies that result by `8`, passes the result
  as the radius of `circle`, and projects the resulting radius-40
  profile through Puri's drawing interface.

The `grap` field belongs to projection rather than evaluation. Normal
view keeps the field visible and projects its value as
`expression → result`; Raw projects only the stored expression.
Compact f64 source values edit as decimal text while continuing to store
the f64 library's byte representation, so changing `pitch` immediately
changes both the `double_pitch` result and the projected circle. This is
intentionally not yet the CAD interaction: it provides a tangible graph
edit, evaluation, and projection loop from which the evaluator can be
redesigned.

## Near-Term Direction

The next useful growth is driven by one interactive geometric
construction, not by filling out a language checklist:

- introduce the smallest geometry values and foreign operations the
  construction needs;
- project evaluated geometry through Puri rather than only text;
- make a direct manipulation write its controlling graph values;
- use the dependency set to reevaluate only affected results if full
  frame evaluation becomes material;
- add absents and evaluation traces as projections over the same graph,
  while keeping Raw as the escape hatch.

The next language work should be forced by manipulating this example:
use `case` when the construction needs conditional structure, refine
patterns from concrete editing experience, and make a thunk or cell
evaluation projection only when the interaction needs one.
Graph-defined macros and general code generation remain out of scope
until a concrete transformation requires them.

## Parked language notes

Working notes, not current work. Take them one at a time, and only
when a construction or editing problem forces the change.

- `quote` exists primarily for `unquote`: walk structure, splice, leave
  the rest unevaluated. A sibling `literal` that copies the same way
  but treats `{unquote: …}` as data would complete the pair. Both are
  ordinary Rust library functions, not evaluator forms.
- An absent from a selected `case` branch is that branch's result, not
  fallthrough. Matching *on* an absent subject is separate and already
  works (the subject is an ordinary value, often one of the stable
  absent cells). Absents stay values, not implicit failure.
- `params` is a list of cells. A user function may reuse library or
  other binders (`left`, `right`, another function's parameter).
  Minting a fresh cell per parameter is an authoring default, not a
  language rule. Shared vocabulary cells *are* a calling convention;
  `add` and `multiply` already share `left` and `right`.
- Patterned function arguments can use the same destructure as `case`.
  A call is already an open record, so a pattern `{left: {bind: x},
  right: {bind: y}}` matches `{function: f, left: a, right: b, …}` and
  ignores `function` and extras. That would replace the ordered
  param-list binding story. `case` remains as local match (same
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
