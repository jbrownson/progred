# Grap and projections

Current implementation, 2026-09-05. Grap is embedded in ordinary GID values.
Its evaluator, library conventions, projection composition, and editor
adaptation have separate responsibilities. Earlier rationale and proposed
language directions are [historical notes](history/projections-notes.md), not
current requirements or verified statements of the owner's intent.

## Packages and libraries

The [Grap runtime](../grap/src/lib.rs) depends only on GID. It implements
lambda/application, environments, callable representations, fuel, source
origins, and stable evaluator absence reasons. It has no editor, geometry,
window, or file services.

The [libraries package](../libraries/src/lib.rs) contains conceptual libraries
as modules: names, text, f64, control, Grap's self-description, geometry,
presentation, layout, and other domains. A `Library` carries definitions and
one partial projection. A library with several forms composes them with
`compose_partials`: first success wins, and the empty composition declines.
The editor composes these library projections in load order above its one
total structural fallback. `Libraries` keeps an insertion-ordered unique map
keyed by stable library identities supplied externally. Repeating a library
identity replaces its entire contribution in place before composition.

Each library stores one sorted definition table. An entry is either an ordinary
`Value` or a shared native definition containing its descriptive `Value` and Rust
implementation. Reading uses the description; calling uses the implementation.
The built-in builders join their data and function declarations once, when
constructing this table. An unnamed native definition has an empty record as
its description. The library's own name record is an ordinary definition under
its library identity, so references and name lookup need no metadata side channel.

[`stack::load`](../progred/src/stack.rs) retains those boundaries and composes
the partial projections and contextual completion providers. A completion provider
receives the query, field/value kind, suggestion/Everything scope, source-qualified
path, and read-only path and cell lookups. Library providers compose in library order;
a projection may supply a local vocabulary on its completion control instead.
There are no root-specific host hooks: root templates and root fields are ordinary
provider decisions about that request. Documents
currently contribute no installed libraries or projections. A Grap function
stored in a document is ordinary reachable data; it is not discovered as
configuration through a reserved cell address.

Library labels are once-minted random `CellId`s. Their readable names are
ordinary GID facts, not identifiers derived from names. Text is an open
`{utf8: blob}` convention; f64 is an open `{f64: eight-byte-blob}` convention.
Additional fields do not invalidate a recognized facet.

## Projection composition

Normal display tries an explicit ordered list of partial functions, followed
by a total structural fallback. A partial can decline; malformed shapes must
remain accessible through a later projection or Raw. Raw uses the structural
fallback alone. Specific domain projections precede general ones. Libraries
contribute these functions explicitly; composition does not depend on registering
them under a shared cell identity. The current host partials are Rust callbacks,
while presentation declarations can apply ordinary Grap callables.

[`ProjectionInput`](../display/src/lib.rs) supplies the environment, value,
scale, writeability, local selection/annotation data, pending state, and
selection targets. The ambient projection is passed explicitly through
recursion. A partial returns `Layout<World, Hover>` with measured-composition
instructions, Puri leaves, host-control requests, and callbacks.

`At`/`descend` extend provenance through a GID step and can prepend contextual
partials. They may supply a missing-child layout at the actual missing
location, without inventing a value. Omitting these specializations uses the
ambient projection and ordinary pending behavior.

The structural fallback follows cells deeply into the selected definition.
Grap expression projections prepend shallow named-cell display at use sites.
Binder/declaration positions and quoted data restore deep display; these
contexts can alternate as expressions nest. Calls use a stored or inline
lambda's declared parameter order when available, then the ordinary order for
extra fields. This is a raw definition lookup, not evaluation of the callable.
Normal record order is named fields alphabetically by display name, with cell
identity breaking ties, then unnamed fields by identity. Raw uses identity order.

`{evaluate: expression}` is a Grap-library projection convention, not evaluator
syntax. It shows the stored expression, an arrow, and the returned value from
a transient read-only root. A result containing another `evaluate` field can
invoke that projection again under the remaining fuel allowance. Ordinary
call-shaped values elsewhere remain editable data until explicitly evaluated.

The presentation library offers an opt-in interpreter for
`{value: source, projection: function}`. Pane views try it only at entry,
following cells through their ordinary definition paths. The document view
leaves the declaration as editable data. It applies the function to the source
as data and projects the result from a transient root using the normal
projection. An absent result reveals the stored source using that same normal
projection. Nested declarations remain data, including inside list or record
panes and computed results. This entry scope does not change the recursive
scope of ordinary contextual partials. The workspace does not interpret this
wrapper. Raw exposes its stored fields in either view.
Explicit `{render: expression}` values retain their ordinary display behavior.

## Lowering and interaction

The [projection runtime](../progred/src/projection/mod.rs) adapts display
layouts to measured boxes and Puri handlers. Its supporting modules separate
[structural fallback](../progred/src/projection/structure.rs),
[layout choices](../progred/src/projection/choices.rs),
[events](../progred/src/projection/events.rs),
[completion](../progred/src/projection/completion.rs), and
[drawing](../progred/src/projection/drawing.rs).

Puri leaves carry text or canvas drawing operations. They do not acquire
selection paths, document editing rules, names, or completion providers.
`LineEdit` and `Completion` are explicit host-control requests above that leaf
boundary. Text and f64 projections supply the stock line control's spelling,
affixes, and Grap write-back function. Progred adapts document operations;
reusable widgets remain consumers of Puri. See [the editor model](model.md).

Callbacks receive mutable world state at dispatch; no projection-action enum
or central reducer sits between a callback and its operation. Generic Grap
event handlers receive GID event values. [`site`](../progred/src/site.rs)
creates temporary selection/annotation capabilities in the current document
and view. `site path` exposes the actual projection path through the
[path library](../libraries/src/path.rs); the local selection getter needs no
address, while the setter accepts one explicitly. Writes are staged and
committed unless the handler explicitly declines or evaluation halts. An ordinary
absent result is still a completed result. Completion continuations
use the same effect interpreter, staged together with the document insertion.

Drawing programs use scoped foreign operations for fills, strokes, paths,
transforms, and clips. A temporary path builder belongs to that synchronous
evaluation. A visible program records once in the frame; hover and painting
share the recording and its source origins. There is no cross-frame canvas memo.

## Evaluation

Grap recognizes these shapes through its vocabulary cells:

```text
{params: [parameter-cells...], body: expression}
{function: callable-expression, parameter-cell: argument-expression, ...}
{closure: {params: [...], body: expression, environment: {...}}}
{ffi: function-cell}
```

Evaluating a lambda captures its lexical environment in a callable value.
Grap-defined calls evaluate every declared argument before the body. Argument
labels are cell identities, so reusing a library parameter cell is meaningful;
its display name does not participate in binding. The ordered parameter list
is the current representation, not a settled general pattern language.

Cell evaluation checks lexical bindings first, then asks `Host::resolve` for one
definition: the document's value, otherwise the first loaded library definition.
Duplicate definitions are tolerated, not merged or composed. With no definition,
evaluation returns missing-cell absent. A native definition's descriptive value
is evaluated without invoking its implementation. A named foreign function
therefore evaluates to its ordinary name record. Scoped capabilities participate
only in calls, never ordinary reads.

Direct calls retain the same `{function: cell, ...arguments}` syntax for Rust
and Grap implementations. To pass a callable reference through an evaluated
argument, use the existing inert `{ffi: cell}` representation; passing a bare
cell evaluates its data. This reference still dispatches in the receiving host
context. It contains neither a native function pointer nor an extra cell definition.
The Fidget example uses such a reference as the argument to `border`.

A direct call uses the same resolver. A native definition invokes its Rust
implementation; an ordinary value is evaluated as the callable.
Every result is definitive, including `{absent: declined}`. A non-callable value
returns a not-callable absent with the offending value; it does not search other
sources for an implementation. Scoped capability functions can override this
lookup. Higher-level operations own any deliberate dispatch or composition.
Projection environments expose one borrowed resolution containing the selected
definition's value, source, and whether it has a native implementation. Call
projections and completion providers can inspect this metadata without evaluating
the callable or mistaking a native function's description for a Grap lambda.

Hosts keep effects in evaluation-local data. Rust capability implementations
wrap selection, annotation, drawing/path writes, and deterministic random
advancement in `context.effect(|| operation)`. The combinator marks the effect,
runs the operation, and returns its result. Arguments and applicability checks
stay outside the closure, so declining before a write remains possible. Reads
do not mark an effect. The context owns one effect counter; each call remembers
its starting count. This includes effects in strict arguments and nested calls,
but excludes effects performed before that call began.

A function must explicitly decline before performing effects. A decline after
an effect halts evaluation with `{absent: effectful-decline, value: cause}` and
prints an error to stderr. This contract remains useful to explicit compositions
and event handlers, independently of cell resolution. It applies to Grap and Rust
functions alike. Other absents remain ordinary results, including a successful
selection clear or a non-callable result.

There are no per-call snapshots or rollback operations. The host stages the
whole editor operation or drawing recording and discards that temporary output
when evaluation halts. Drawing uses local vectors, and the random stream uses
local scalar state. The interpreter does not mutate the live editor. Rust FFIs
must mark their observable writes and keep their effects local; external I/O
is not reversible through this facility. Fuel is never restored.

Rust foreign functions receive raw call fields, the calling environment, and
a live evaluation context. They choose which operands to evaluate and in what
environment. Strict arithmetic evaluates all its operands; control functions
can evaluate only the chosen branch. Blobs, lists, and unrecognized records
are inert: the evaluator does not search ordinary containers for expressions.
A function's returned value is not evaluated again.

The registered `evaluate` function is distinct from the projection field. It
accepts an explicit environment value and asks the evaluator to interpret its
raw expression there. It is an ordinary library function.

## Control functions and absents

The [control library](../libraries/src/control.rs) supplies structural matching
and quote/unquote as ordinary Rust functions using that evaluator interface.
`match` evaluates its subject once, then tries ordered cases. Record patterns
are open; list patterns are exact and ordered; `{bind: cell}` captures a value.
Repeated binders require equal captures. Only the selected expression runs,
in the caller's environment extended by captures. Its absent result is final,
not a request to try another case. There is no implicit match-subject binding.

`quote` walks the original raw structure, replacing `{unquote: expression}`
leaves by evaluating their expressions in the caller's environment. It never
revisits a spliced result. Outside that traversal, an unquote-shaped record is
ordinary data. There is no evaluator-level quote/literal form.

Semantic failure is an open GID record:

```text
{absent: reason-cell, ...occurrence-details...}
```

The reason cell supplies names and other static metadata. Occurrence fields
can identify a missing cell, invalid value, or cycle. Code inspects identities,
not human-readable diagnostic text. Hosts and tests inspect the returned value;
there is no parallel Rust diagnostics list that can reject a successful result.

An unmatched `match` preserves one mismatch unchanged or combines several as
`{absent: no-alternative, causes: [...]}`. This is an ordinary absent, not an
implicit request to try another function definition. A handler using `match`
can explicitly decline with a final catch-all case. A selected expression's
result remains definitive within that match.

Fuel exhaustion halts immediately; at the public boundary it is still an absent
value, with `Evaluation.completed` recording that execution did not finish.
An effectful decline also halts rather than returning normally.
Returning that same value normally still counts as completed execution.
Drawing-source origins are separate from failures.

## Equivalent host representations

The runtime can carry unboxed f64 values and containers with lowered children.
The f64 encoder/decoder live with the runtime carrier and are re-exported by
the f64 library. They accelerate an ordinary convention without adding numeric
syntax or new GID primitives. An enriched source number retains its original
value so a pass-through preserves unrelated fields. Generic boundaries
materialize ordinary GID values.

Environment lookups and structural matching operate on these equivalent host
representations. Lowering must be transparent to Grap behavior, including
when a value comes through a cell or a constructed container. Origin metadata
preserves both the definition source and the relative path; it cannot identify
a definition by cell alone. See [source identity](model.md#occurrences-definitions-and-views).

## Examples and deferred work

[grap-demo.gid](../examples/grap-demo.gid) exercises calls, closures, matching,
quote/unquote, explicit evaluation, and absents. [iop-tree.gid](../examples/iop-tree.gid)
projects an interpreted tree drawing alongside editable source. Headless tests
exercise these paths through the production library set.

General invalidation, address-lifetime questions, and hidden active insertions
are [deferred](deferred.md). Other language ideas in historical notes remain
proposals. The old Rust-to-wasm compiler/host spike was removed; its source
remains in Git history. There is no active Wasmtime projection host in this tree.
