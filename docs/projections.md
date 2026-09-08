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

Progred's [libraries module](../progred/src/libraries/mod.rs) contains conceptual libraries
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

Normal display uses one composition of ordered partial functions, followed
by a total structural fallback. A partial can decline; malformed shapes must
remain accessible through a later projection or Raw. Raw uses the structural
fallback alone. Specific domain projections precede general ones. Libraries
contribute these functions explicitly; composition does not depend on registering
them under a shared cell identity. The current host partials are Rust callbacks,
while presentation declarations can apply ordinary Grap callables.

[`ProjectionInput`](../progred/src/display/mod.rs) supplies the environment, value,
scale, writeability, local selection/annotation data, pending state, and
selection targets. Its `default_projection` is one composed partial function,
passed explicitly through recursion. A partial returns a `Layout<World, Hover>`
program that calls the layout builder with box operations, Puri leaves, and
opaque widget/preparation functions.

`At`/`descend` extend provenance and accept independent optional replacements
for the projection at their target and the default passed to descendants.
Omitting either inherits it; supplying one replaces it without implicit
composition. The total structural fallback still handles a declined result.
`descend_local`/`at_local` compose a custom partial before the default only at
the target. `at_scoped` passes that composition both here and below, so callers
can intentionally establish a scope. Both build ordinary preparation functions,
not path-bearing layout opcodes.

Partials receive `Option<&Value>`: `None` means a missing location, not a GID
absent or an empty string. `descend` offers the resolved value or its absence to
the chosen partial; if it declines, the fallback renders structure for `Some`
or the standard empty picker for `None`. There is no separate `missing` parameter
and no fabricated value. The same source-qualified location supplies selection
and writeability in either case.

The lambda-name partial shows `λ` when the name is missing and unselected.
Activation selects that missing location for entry. Once selected, the partial
declines and the ordinary empty picker takes over; only committing creates the
field. Neither pointer activation nor navigation supplies picker state. The
picker uses defaults for an ordinary selection, including an empty payload.
The name library supplies text suggestions at name fields. Existing names
use unquoted line editors, like parameter names. An explicitly empty name stays
an empty editor, distinct from the missing name's `λ` marker. Ordinary string
projections retain their quotes.
Library completion providers still compose separately, so multiple libraries
can contribute offers to one picker.

Grap contributes a `new lambda` value completion (aliases `lambda` and `λ`) to
the universal picker. It inserts `{params: []}` and selects the missing body.
The lambda projection accepts that unfinished shape and descends into the body
to show its ordinary empty picker; neither a body nor a name is fabricated.

The shared `structure::list(Some(child_projection))` combinator explicitly
applies a partial at each immediate element. Standard lists use the same
renderer with `None`. Lambda parameters, match cases, let/where bindings, and
do expressions each supply their element projection this way.
`structure::record` takes a function from each field identity to an optional
child partial. Standard records use the same renderer with no field overrides;
record layouts also share `record_heads` geometry with ordered call arguments.
Patterns establish an explicit binder-projection scope. Ordinary container
recursion then finds nested binders while text and numeric facets keep their
normal projections; no duplicate pattern-specific structural walk is needed.
The scope does not leak to siblings outside the pattern. List separators retain
host-provided insertion targets.

The structural fallback follows cells deeply into the selected definition.
Grap expression projections request shallow named-cell display at direct use
sites. Compound forms choose their own children; inert containers, declaration
metadata, and quoted data use the normal deep structural fallback. In lambda parameters, direct
`let`/`where` binders, and pattern binders, a cell whose definition contains
only a text name projects as `(name)`: the usual cell parentheses surround an
unquoted line editor at the real `Follow` → `name` path. This contextual
projection is not in the default stack. Extra definition fields, malformed or
missing names, and active field insertion retain structural display. Ordinary
cells and quoted data are unchanged; use sites remain shallow references.
Calls use a stored or inline
lambda's declared parameter order when available, then the ordinary order for
extra fields. This is a raw definition lookup, not evaluation of the callable.
Normal record order is named fields alphabetically by display name, with cell
identity breaking ties, then unnamed fields by identity. Raw uses identity order.

The Fidget library projects field arithmetic as infix expressions, other scalar
operations as named argument groups, and coordinates as shallow references.
Grouping preserves the expression tree, including right-nested operations of
equal precedence; it never reassociates floating-point arithmetic. Names come
from ordinary definitions. Operands retain their stored paths and stock editing
controls, while an operator targets its whole expression. Named fields expose
their editable name beside the formula. Incomplete forms or extra fields decline
to structural display; translation keeps its explicit named parameters. These
are source projections only, independent of the opt-in rendered viewport.

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
panes and computed results. Entry intentionally follows cells to the first
non-cell; this entry policy is separate from explicit projection scopes.
The workspace does not interpret this
wrapper. Raw exposes its stored fields in either view.
Explicit `{render: expression}` values retain their ordinary display behavior.

Assigned-size panes instead use `{value: source, viewport: function}`. The editor
recognizes this contract at pane entry and passes the settled logical `width`
and `height` along with `value`. The result goes through the same transient
layout lowering and handler machinery as other computed content. There is no
pane-size lookup FFI and no size field on every ordinary projection input.
The pane supplies the clip and no padding or document scroll handler. The
function may provide its own interactions, such as Fidget orbit and zoom.
An absent result exposes the stored source; Raw exposes the whole declaration
in the ordinary scrolling view. Malformed declarations and cell cycles fall
back to normal projection.

## Lowering and interaction

The [projection runtime](../progred/src/projection/mod.rs) resolves document
locations and supplies borrowed widget inputs. The
[box interpreter](../progred/src/display/measure.rs) composes measurements without
interpreting document traversal or control requests. Supporting modules separate
[structural fallback](../progred/src/projection/structure.rs),
[Grap event adaptation](../progred/src/libraries/layout/events.rs),
[completion](../progred/src/projection/completion.rs), and
[drawing](../progred/src/projection/drawing.rs).
The [native completion card](../progred/src/display/widget/completion.rs) owns its
row interaction, keyboard navigation, and scroll composition. The app adapter
only supplies offers, query state, document callbacks, and popup placement.
The generic [layout choice engine](../ui/measured/src/choices.rs) belongs to
`measured`, independently of those editor adaptations.

Grap's `descend` and `at` forms decode through the
[path library](../progred/src/libraries/path.rs), also used by site and selection
capabilities. Field keys, list positions, and source-qualified definition
follows have one encoder/decoder. They produce ordinary preparation functions
using an explicit projection scope; layout has no parallel path vocabulary or
traversal opcodes. Preparation interleaves descendant projection and measurement
before choices resolve. Shared children prepare once per frame, and only chosen
placement continuations contribute interaction and ink.

Puri leaves carry text or canvas drawing operations. They do not acquire
selection paths, document editing rules, names, or completion providers.
The stock [line widget](../progred/src/display/widget/line.rs) is an ordinary native
function. Text and number projections supply its spelling, affixes, and
conversion callback; `Layout::widget` carries the resulting measurement
program without inspecting its props. Placement returns a hover continuation;
running it contributes native render continuations, handlers, the hover claim,
and a navigation transition through
the [widget output interface](../progred/src/display/widget.rs). These are Progred
widgets, so their handlers receive `&mut Editor` and call ordinary
[editing helpers](../progred/src/editing.rs); no generic-world callback dictionary
is installed at each location. The [line adapter](../progred/src/projection/line_control.rs)
applies editing operations and conversion, with undo grouping owned by Progred. The current
handler owns conversion, not the selection payload. The line library adapts
Grap conversions explicitly; native controls do not round-trip through Grap.
Completion uses an ordinary native widget factory with explicit kind/provider
inputs. The app adapter constructs document-specific offers and pending
state; the reusable card owns row ink, navigation, and scrolling. Drawing-program
widgets directly use the app's evaluation/source attribution helpers. Both
return the same measured `HoverPass` as other widgets, not control opcodes.
The app's `Placed` aliases that continuation. Running it returns `Fragment`
(`Ready` in the app), with paint and handlers as independent outputs.
Reusable widgets remain consumers of Puri. See [the editor model](model.md).

Delimiter handles use ordinary [side widgets](../progred/src/display/widget/delimiter.rs).
An ordinary row lays out fixed-width sides, which adopt its available height
during placement; selection and picking are explicit `selectable_widget`
composition inside the stretch, not interpreter behavior.
The standard record/list/cell projections request `selectable_bracket`. The
low-level `bracket` constructor, including its Grap layout encoding, contributes
only ink and geometry. The same settled hover drives native handler activation,
so retained hover still selects the highlighted target rather than re-hit-testing
the click position.

Click, activation, and picking likewise compose ordinary
[native interaction functions](../progred/src/display/widget/interaction.rs).
`widget::before` prepares a placement continuation, which can contribute ink,
claims, or handlers before any child; the layout interpreter only composes it.
`widget::after` contributes after the child through the same output interface.
The standard border is an ordinary native `after` decorator, with no border
opcode in layout or border rendering rule in the interpreter.
There are no click/activate/pick enum cases. Existing Grap layout constructors
decode to these same functions. The current site's value and the host's pick
capability are separate inputs, so an explicit pick target need not equal the
value being projected. Neither path changes pointer-handler precedence.
Hover claims and occlusion use the same native decorator interface. Hover
feedback is an explicit, independent decorator used by insert/collapse handles,
not a hidden policy selected by inspecting the target's enum variant. Native
widgets request site state only when needed and retain a whole-widget rendering
callback, not one deferred allocation per canvas operation. Empty outlines also
use a native widget rather than an interpreter case.

Grap's `on_event` adapter belongs to the layout library and builds an ordinary
`before` decorator too; there is no `Layout::OnEvent`. One handler encodes the
incoming Puri event and calls the supplied site-scoped interpreter. The adapter
owns the event vocabulary; the app owns staging and committing effects. Native
widgets bypass that interpretation and use their callbacks directly.

Scrolling uses an ordinary native placement handler too. Fidget's handler
closes over a requested annotation-write capability and explicitly applies its
camera update; layout does not interpret a state-scroll result. Acceptance and
the unused displacement use Puri's `ScrollOutcome`, independently of writes.
Document scrolling and widget scrolling share the same conversion of pixel,
line, and page input and its remainder. The conversion knows no document state.

Callbacks receive mutable world state at dispatch; no projection-action enum
or central reducer sits between a callback and its operation. Generic Grap
event handlers receive GID event values. [`site`](../progred/src/site.rs)
creates temporary selection/annotation capabilities in the current document
and view. `site path` exposes the actual projection path through the
[path library](../progred/src/libraries/path.rs); the local selection getter needs no
address, while the setter accepts one explicitly. Writes are staged and
committed unless the handler explicitly declines or evaluation halts. An ordinary
absent result is still a completed result. Completion continuations
use the same effect interpreter, staged together with the document insertion.

Drawing programs use scoped foreign operations for fills, strokes, paths,
transforms, and clips. A temporary path builder belongs to that synchronous
evaluation. A visible program records once in the frame; hover and painting
share the recording and its source origins. There is no cross-frame canvas memo.

## Scoped layout programs

The layout library's `layout program` function takes a raw `expression` and
returns `{layout program: closure}`. This is an ordinary Grap closure with its
lexical environment, not an opaque native value. The normal partial recognizes
that record and runs the closure with a borrowed `ForeignOverlay` capturing a
Rust layout output buffer. No evaluator types or syntax were added.

Within that scope, `row`, `col`, `overlay`, and `alternatives` take a raw
`children` expression. They evaluate it once into a fresh child buffer, then
emit the corresponding native layout. `do`, ordinary function calls, and loops
can produce that sequence; a list of expressions remains inert unless a control
function evaluates it. `pad`, `bracket`, and interaction wrappers take a raw
`child` expression that must emit exactly one child. No buffer borrow is held
while evaluating a body. Successful operations return the ordinary empty record;
their useful output remains in Rust.

Leaf/recursion capabilities include `text`, `slot`, `descend`, `at`, and
`transient`. `canvas` accepts width/ascent/descent and a drawing-program value;
it does not construct a list of draw commands. Its optional fuel argument
configures the later drawing evaluation, as with the stored drawing convention.
Row/column gap defaults to zero, column baseline to zero, padding sides to zero,
and text paint to the ink face. Other required inputs are validated. An invalid
builder call, multiple root emissions, or evaluator halt discards all output;
an invalid call cannot leave a partial layout even if `do` ignores its result.
An intentionally empty layout can emit an empty row or column.

The explicit boundary matters for composition: pane/viewport projection
functions still return ordinary values. The existing `border` combinator wraps
a layout-program record like any other result. Neither it nor other existing
value-returning functions changes meaning inside a secretly installed scope.
Stored GID layout descriptions remain supported. The new path avoids building
and decoding those descriptions per node, but still allocates the chosen Rust
layout representation and evaluates Grap code. It is not a cache or a guarantee
of improved speed for arbitrary programs.

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
The Fidget example uses such a reference for its viewport function.

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

The [control library](../progred/src/libraries/control.rs) supplies structural matching
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
