# Grap and Projections

Decision record, 2026-08-08. This supersedes the 2026-07-25 decision
to make Rust-to-wasm plugins the first projection language. The wasm
spike remains in the tree as proven infrastructure, but it is no
longer on the application's live f64 projection path.

## The Decision

Bootstrap Progred with Grap, a small strict language embedded directly
in the existing graph data. Grap is not another syntax tree and adds
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
  being designed before a useful evaluator existed. The bootstrap has
  only one-pass `quote`/`unquote`, enough for functions and foreign
  functions to construct graph data without accidentally evaluating it.
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

Core Grap defines five syntax identities: `function`, `params`, `body`,
`quote`, and `unquote`. They distinguish function definitions, calls,
and quotation from ordinary records. Core Grap has no number or
geometry type and no arithmetic or geometry operation.

Progred's projection layer separately defines the `grap` field. The
ordinary record and its `grap` label remain visible, but the projection
for the value under that label shows the stored expression through its
ordinary editable projection, followed by `→` and the returned `Value`
projected as a read-only normal form. Normal view therefore shows
`{grap: expression → result}`, while Raw shows
`{grap: stored-expression}`. The arrow is projection chrome, not graph
data. `grap` is not a Grap evaluator form, so the evaluator can be used
without Progred and cannot observe the field.

A function is a record requiring two semantic fields:

```text
{
  params: [x, y],
  body: ...,
}
```

The parameter values are cells. The list establishes order, and each
cell is lexically bound to the value under the matching call field while
evaluating the body. The definition is an open record pattern:
unrelated fields do not stop the record from being a function. A
function may be inline and anonymous or be the value of a cell; naming
and recursive reference need no additional language identity.

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
fields are valid graph data. Evaluation reports fields the selected
function did not consume for tooling and future pattern composition.
The result arm under `grap` is derived and read-only; the expression
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
can therefore show a function definition or expression as editable
structure in one part of a document while a `grap` field elsewhere
references that same cell and shows both that ordinary cell projection
and its result. Because the expression arm is ordinary, hovering it can
highlight the cell's other projections. The returned value goes through
the same text, number, geometry, cell, list, and record projections as
stored data. That subtree is marked as Grap normal form, so a returned
value which itself contains a `grap` field is data rather than another
request to evaluate. Derived children are currently read-only and map
selection back to the stored `grap` field value rather than pretending
to have document paths.

Evaluating a cell is transparent:

1. A lexical binding with that cell identity wins.
2. A cell registered by a library as a foreign function produces its
   host implementation.
3. Otherwise the cell is resolved through the caller's document-over-
   library source and its value is evaluated.

This is one dependency made honest, not a special reference/value
mode stored on cells. A parameter may have a name or even a document
value; within its function body the lexical binding wins because that
identity is the parameter.

Blobs, lists, and unrecognized records are inert data. The evaluator
does not search them recursively for expressions. A record containing
the fixed `function` label is a call, and a record containing `params`
or `body` is treated as a function definition. The selected function
drives recursion: Grap application evaluates each declared argument
field before evaluating the body; the f64 multiply implementation
evaluates its left and right fields; an unrelated record evaluates to
itself without inspecting its children. Closures capture the lexical
environment in which their definition is evaluated.

This is strict call-by-value, not eager traversal of all graph data.
Programs that need deferred work can return inert data such as
`{isa: thunk, body: ...}`. Grap need not know that convention. A
projection can show the thunk as an ellipsis and evaluate its body only
when the user expands it. Repeated forcing initially repeats the work;
sharing and memoization are optimizations rather than bootstrap
semantics.

Quotation is the way to produce data that resembles Grap syntax. A
`quote` walks its body once. Each `unquote` it encounters is replaced by
the evaluation of that unquote's body, and the inserted result is not
walked again. An unquote outside a quote has no special status; it is an
ordinary inert record. Quotation does not follow cell links: a cell in
the template remains a link. An explicit unquote around that cell
evaluates it and copies the resulting value into the constructed data.
This is copy into the result, never mutation of the referenced cell.

These conventions match what is present, not what is absent. Record
patterns are open unless a particular domain explicitly says
otherwise. The current compact f64 and circle views are deliberately
stricter than semantic recognition: they replace a whole record only
when every field in that record belongs to the displayed facet.
Otherwise the structural view remains visible, even though Grap and
the relevant library can still use the recognized facet.

Every external cell read is collected as a dependency. The set is
reported even when evaluation produces an error value, ready for future
precise invalidation. Every evaluation also has explicit fuel, and cell
cycles receive a stable error value. Invalid Grap never damages the
underlying document: its error value is projected like any other normal
form, and the stored expression remains editable in Raw or wherever
the same expression cell is projected outside a `grap` field.

The evaluator lives in its own `grap` crate. It depends on the graph
core and the shared Grap error and name conventions, but knows no f64,
geometry, UI, file, or Linebender concepts. `grap-f64` and
`grap-geometry` are separate libraries composed by the application.
Their identities and ordinary graph-side values live in the built-in
library cells; their host-side implementations live in the generic
foreign-function registry.

A registered Rust implementation declares the call fields it consumes,
receives their evaluated values, and returns a `Value`, just as
evaluation of a graph function body ultimately does. The evaluator does
not impose a host-language `Result` distinction at that boundary. The
current registry is bootstrap machinery, not the intended final model:
the emerging model is one `Value -> Value` evaluator composed from
ordered pattern-matching clauses, some written in Rust and some in the
graph. A dispatch index may later optimize that composition without
becoming part of its semantics. Merely closing over the current lookup
would be cosmetic. A genuine clause protocol must give a matched clause
recursive evaluation and say whether a returned error means “decline;
try another clause” or “this clause handled the value and failed.” The
current table can then be wrapped as the first Rust-authored clause;
graph-side bootstrapping is not required.

Every evaluation returns a `Value`, including malformed programs,
missing cells, cycles, and exhausted fuel. Core evaluator failures and
library-specific failures are stable error-cell values. Host-facing
diagnostics accompany core failures with occurrence-specific detail,
such as which cell was missing, without introducing a separate failure
channel into Grap or changing Grap control flow. When an evaluated call
returns one of those cells, the projection shows its ordinary name as
the result.

The bootstrap f64 and geometry libraries define stable library cells
for their failure modes and return those identities as values: several
semantically distinct custom nulls, not freshly allocated error
occurrences. Each sentinel's library value is a record containing
`isa: error`. The general `isa` relation lives in the independent
`progred-isa` library: it is a convention over graph data, not part of
Grap or `progred-graph`. The error library owns only the `error`
classification and uses that relation. Additional static facts can be
added as fields on each sentinel's record. Error meaning remains
library data rather than an evaluator feature.

## First Vertical Slice

For a `grap` field, normal projection shows the stored expression, an
arrow, and its normal form—an f64 as text, a circle as native vector
drawing, and arbitrary graph data structurally. The enclosing record
remains ordinary visible data. `grap-demo.gid` is
the focused interactive playground: three editable f64 cells feed
direct foreign calls, nested calls, a graph-defined function, and a
circle; it also keeps extra call metadata in Raw, demonstrates
quote/unquote and inert returned data, and shows stable type,
missing-argument, and not-callable error cells as ordinary projected
results. The demo projects one graph expression cell both directly and
by reference under `grap`, making their shared identity visible through
hover while the latter also carries its derived result.

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
`expression → normal-form`; Raw projects only the stored expression.
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
- add errors and evaluation traces as projections over the same graph,
  while keeping Raw as the escape hatch.

The next language work should be forced by manipulating this example:
replace the special Rust registry with ordered pattern-function
composition, decide how graph patterns bind values, and make a thunk or
cell evaluation projection only when the interactive construction needs
one. Quotation is not a commitment to a general macro system; general
code generation remains out of scope for the bootstrap.

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
