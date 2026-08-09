# Grap and Projections

Decision record, 2026-08-08. This supersedes the 2026-07-25 decision
to make Rust-to-wasm plugins the first projection language. The wasm
spike remains in the tree as proven infrastructure, but it is no
longer on the application's live f64 projection path.

## The Decision

Bootstrap Progred with Grap, a small strict language embedded directly
in the existing graph data. Grap is not another syntax tree and adds
nothing to `Value`: records, lists, blobs, strings, and cells remain the
whole data model. A fixed library gives a few cell identities meaning,
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

- Parameters were inferred from free cells. They are now an explicit,
  ordered list.
- Cell resolution changed meaning based on whether the cell happened
  to have a stored value. It is now ordinary lexical lookup followed
  by document/library lookup.
- Templates, macros, hygiene, and mint-on-instantiation were being
  designed before a useful evaluator existed. None is in the bootstrap
  language.
- Grap was being weighed as a general replacement for existing
  languages. Its current job is smaller: make the system immediately
  live, then provide the substrate from which richer projections can
  be built.

## Representation

Semantic labels are library cell IDs, not strings. The IDs are
once-minted random 128-bit cell identities, checked in as library
facts; they are not derived from, or hashes of, their names. Names are
presentation metadata supplied by each library.

Core Grap defines only three identities: `function`, `params`, and
`body`. They distinguish function definitions and calls from ordinary
records. Core Grap has no number or geometry type and no arithmetic or
geometry operation.

A function is a record with exactly two fields:

```text
{
  params: [x, y],
  body: ...,
}
```

The parameter values are cells. Their list establishes arity and
order. The function record may be inline and anonymous or be the value
of a cell; naming and recursive reference need no additional language
identity. The fixed `function` cell cannot itself be a parameter,
because that label is the call record's one reserved slot.

A call is a record with a `function` field and one field per argument.
Argument labels are the function's parameter cells:

```text
{
  function: sum,
  x: ...,
  y: ...,
}
```

The apparent names above are binder sugar in gid notation. Matching is
by cell identity. Renaming a parameter changes no program reference,
and there is no parallel symbol-ID system.

Numbers remain a library convention rather than a data-model variant.
The separate f64 library represents an f64 as eight little-endian bytes
under its `f64` label. It defines strict binary `add` and `multiply`
calls using its `left` and `right` parameter cells, and registers their
implementations as Rust foreign functions. The geometry library owns
`circle` and `radius`; its Rust-backed circle constructor consumes the
f64 library's representation. Neither library changes Grap or
`Value`.

## Evaluation

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

Ordinary values evaluate to themselves. Only a record containing the
fixed `function` label is a call, and only a record containing the
fixed function-definition labels is a definition. Calls are
call-by-value. Closures capture the lexical environment in which their
definition is evaluated.

Every external cell read is collected as a dependency. The set is
reported even when evaluation fails, ready for future precise
invalidation. Every evaluation also has explicit fuel, and direct cell
alias cycles receive a specific error. Invalid Grap never hides or
damages the underlying document: a failed projection simply declines,
and the raw record remains editable.

The evaluator lives in its own `grap` crate and depends only on
`progred_graph`. It knows the function representation and a generic
foreign-function registry, but no f64, geometry, UI, file, or
Linebender concepts. `grap-f64` and `grap-geometry` are separate
libraries composed by the application. Their graph-side identities
and metadata live in the built-in library cells; their host-side
implementations live in the foreign-function registry.

A registered foreign function consumes evaluated values and returns
one `Value`. The Grap evaluator does not impose a host-language
`Result` distinction on that value.

The bootstrap f64 and geometry libraries define stable library cells
for their failure modes and return those identities as values: several
semantically distinct custom nulls, not freshly allocated error
occurrences. Each sentinel's library value is a record containing
`isa: error`. The general `isa` relation lives in the independent
`progred-isa` library: it is a convention over graph data, not part of
Grap or `progred_graph`. The error library owns only the `error`
classification and uses that relation. Additional static metadata can
be added as fields on each sentinel's record. Error meaning remains
library data rather than an evaluator feature.

## First Vertical Slice

The raw projection asks Grap to evaluate candidate records, then
projects a successful f64 result as text or a circle result as native
vector drawing. The checked-in sample is the small end-to-end
construction:

- `pitch` is a cell containing f64 `2.5`.
- `double` is a Grap function with an explicit `amount` parameter. Its
  body calls the f64 library's Rust-backed `multiply` with `amount`
  and f64 `2`.
- the roof contains a call to `double`, passing `pitch`; its projected
  result is `5`.
- a nested expression multiplies that result by `8`, passes the result
  as the radius of `circle`, and projects the resulting radius-40
  profile through Puri's drawing interface.

Raw mode shows the complete function, call, and number records. This
is intentionally not yet the CAD interaction: it proves the shorter
loop — graph edit, evaluation, vector projection — before direct
manipulation and a real construction vocabulary are layered on it.

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

Conditionals, local bindings, recursion policy, richer errors, and
collections should arrive only when the construction demands them.
Macros and general code generation remain explicitly out of scope for
the bootstrap.

## The Superseded Wasm Spike

The 2026-07-25 spike proved that Progred can compile Rust to wasm over
pipes and safely host pure projection plugins in Wasmtime. ABI 1 uses
three exports (`abi_version`, `alloc`, and `project`) plus linear
memory. Calls receive no imports, use a fresh Store, and are bounded by
epoch interruption. The compiler service invokes an explicitly chosen
rustup toolchain in a per-call temporary directory and returns
structured rustc diagnostics.

That code, the f64 guest, and their tests are retained. They are useful
evidence for a future foreign-language boundary and are not interfering
with Grap. The app no longer compiles or loads the f64 guest at startup,
and the active projection no longer depends on Wasmtime. Removing the
dormant dependency and spike is a separate cleanup decision, not part
of establishing Grap.
