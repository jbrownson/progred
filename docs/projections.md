# Grap and projections

Current implementation, 2026-09-05. Grap is embedded in ordinary GID values.
Its evaluator, library conventions, projection composition, and editor
adaptation have separate responsibilities. Earlier rationale and proposed
language directions are [historical notes](history/projections-notes.md), not
current requirements or verified statements of the owner's intent.

## Packages and libraries

The [Grap runtime](../grap/src/lib.rs) depends only on GID. It implements
lambda/application, value wrappers, environments, callable representations, fuel, source
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
path, Raw mode, read-only path and cell lookups, and lazy enumeration of defined cells.
Library providers compose in library order;
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

Explicit Rust memo boundaries can run whole Grap evaluations through the
[dependency-tracking adapter](incremental.md#grap-and-foreign-calls).
Ordinary projection/evaluation remains uncached. Foreign functions opt into
tracked reads; unknown calls and unrecorded effects prevent reuse.

Normal display uses one composition of ordered partial functions, followed
by a total structural fallback. A partial can decline; malformed shapes must
remain accessible through a later projection or Raw. Raw uses the structural
fallback alone. Specific domain projections precede general ones. Libraries
contribute these functions explicitly; composition does not depend on registering
them under a shared cell identity. The current host partials are Rust callbacks,
while presentation declarations can apply ordinary Grap callables.

Recognition checks the fields a partial uses, not the absence of unrelated
fields. Extra metadata may remain unshown in a compact projection; Raw exposes
the stored record. Missing or malformed required contents decline normally,
without reserving the record or blocking later partials. Explicit mutually
exclusive tags within a convention still reject conflicts. Active field
insertion can use the general presentation to keep its picker visible.

[`ProjectionInput`](../progred/src/display/mod.rs) supplies the environment, value,
scale, writeability, local selection/annotation data, pending state, and
selection targets. Its `default_projection` is one composed partial function,
passed explicitly through recursion. A partial returns a `Layout<World, Hover>`
program that calls the layout builder with box operations, Puri leaves, and
opaque widget/preparation functions.

`at`/`descend`/`jump` accept independent optional replacements
for the projection at their target and the default passed to descendants.
Omitting either inherits it; supplying one replaces it without implicit
composition. The total structural fallback still handles a declined result.
`descend_local`/`descend_path_local` compose a custom partial before the default
only at the target. `descend_path_scoped` passes that composition both here and
below. `at_with_projection` accepts the same explicit partial compositions.
These are ordinary preparation functions, not path-bearing layout opcodes.

`descend` follows one step of the displayed structure; `descend_path` follows
several without projecting intermediate containers. Both preserve the current
location interpretation. `jump(steps, document_path)` extends the occurrence
path by `steps` but reads from an absolute, source-qualified document path.
Its descendants conject to that source. `jump_with_conject` supplies the lower
level function, `(&projection_suffix, &document_path) -> Option<Path>`, used by
both projection reads and later edits. No inverse is required.

`at(steps, value)` instead projects the supplied value with no document source.
Its conject returns `None`, including for ordinary descendants and cell follows.
An explicit nested jump can establish a source. Existing stored-child partials
use `descend_path`, not `at` with a copy of the source value. Computed results
use the same `at` combinator, with a `Key(presentation::RESULT)` occurrence
step distinguishing the result from its producing expression. This is a
projection path, not a fabricated field or writable document location.

Partials receive `Option<&Value>`: `None` means a missing location, not a GID
absent or an empty string. `descend` offers the resolved value or its absence to
the chosen partial; if it declines, the fallback renders structure for `Some`
or the standard empty picker for `None`. There is no separate `missing` parameter
and no fabricated value. A missing value at a real source remains writable;
having no source is distinct and does not offer document editing. Selection
identity always belongs to the displayed occurrence.

The common preparation boundary for `descend`, `at`, and `jump` supplies
selected-value copy/cut and fold handlers below the projected widget's own
handlers. Copy captures the actual projected value, not a document lookup.
Fold captures the value's collapse default and updates only occurrence-local
annotations; even an occurrence with no document source can copy and fold.
Only the deletion half of cut requires a document destination.

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

The expression-use partial also handles missing values with a focused completion
provider. It combines statically visible bindings and function calls with the
loaded libraries' value constructors; general cell search remains behind `…`.
Only explicit expression children use this provider, not quoted data, parameter
declarations, or ordinary structural containers. Provider composition is lazy:
the projection obtains the library provider from `Env`, but offers and scope are
read only for the active picker.
At a missing expression, the default partial gets the first opportunity to
present selection-driven interactions (such as the new-color picker), before
falling back to expression completion.

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
The scope does not leak to siblings outside the pattern.

Bracketed and column lists prepare shared children with `structure::list_items`,
including the pending insertion, then use `structure::list_with` for each
presentation. The item callback
wraps each projected child; the separator callback chooses which boundaries
to show and supplies their presentation, receiving an optional hover identity
for feedback. The common combinator disables insertion beside a pending or in
read-only data and activates a separator by selecting an ordinary missing
element position. There is no separate host insertion capability or hover kind.
Separators adopt their container's offered width before both interaction and
painting: a column's width, or their intrinsic width in a row. Each call returns
one interleaved sequence, with no alternatives or arrangement policy inside it.
The bracketed list explicitly calls it twice, using commas for a row and
clickable gaps for a column, then combines those layouts as alternatives.
Children are projected and measured once across those calls. Commas and hover
lines supply no editing handlers of their own.

The structural fallback follows cells deeply into the selected definition.
Grap expression projections request shallow named-cell display at direct use
sites. Without a valid text name, the partial declines and the cell projects
deeply, keeping its parentheses and editable definition. This is a presentation
choice, not an inference about lexical bindings or their runtime values.
Compound forms choose their own children; inert containers, declaration
metadata, and quoted data use the normal deep structural fallback. In lambda parameters, direct
`let`/`where` binders, and pattern binders, a cell whose definition contains
a text name projects as `(name)`: the usual cell parentheses surround an
unquoted line editor at the real `Follow` → `name` path. This contextual
projection is not in the default stack. Unrelated definition fields do not
block it; malformed or missing names and active field insertion decline to the
ordinary projection. Ordinary cells and quoted data are unchanged; named use
sites remain shallow references.
Calls use a stored or inline
lambda's declared parameter order when available, then the ordinary order for
extra fields. Missing declared arguments appear as ordinary editable empty slots
at their real field paths; they remain absent from the document until edited.
An active missing argument stays in its declared position rather than gaining a
second trailing row. This is a raw definition lookup, not evaluation of the callable.
Numeric libraries decorate their math calls' function references with the same
representation subscript used by literals. Both names come from definitions;
the decorated label still selects the call's `function` field. A comparison's
subscript identifies its operand representation, not its boolean result.
Numeric facets explicitly include an optional stored name beside the number,
separated by spacing rather than binding or record-field punctuation. The shared
name decorator descends to the real `name` field with an unquoted text editor;
the number retains its own editor and scrub target. This is opt-in library
composition, not automatic merging of every matching facet's projection.
Normal record order is named fields alphabetically by display name, with cell
identity breaking ties, then unnamed fields by identity. Raw uses identity order.

Grap's `{name: text, value: expression}` wrapper projects as `name = expression`.
The name slot is always present: a missing name uses the normal empty picker,
with the name library's string completions. The expression uses the local
shallow-reference projection. Other metadata does not block recognition, and
both editors retain their stored field paths. This is a named expression, not a
`let` binding; the evaluator ignores its name. Named numeric facets remain
independent of this Grap-specific wrapper.

Within Grap's library, calls, lambdas, and value wrappers are tried in evaluator
precedence order. Each remains an ordinary partial: malformed contents decline
just like a missing required field, allowing the next partial to try. Recognition
does not reserve a record or force structural fallback.

The Fidget library projects field arithmetic as infix expressions, other scalar
operations as named argument groups, and coordinates as shallow references.
Grouping preserves the expression tree, including right-nested operations of
equal precedence; it never reassociates floating-point arithmetic. Names come
from ordinary definitions. Operands retain their stored paths and stock editing
controls, while an operator targets its whole expression. Named fields expose
their editable name beside the formula. Unrelated fields on an operation or its
operand record do not block notation or change grouping. Incomplete forms and
conflicting operation tags decline; translation keeps its explicit named
parameters. These are source projections only, independent of the opt-in
rendered viewport.

`{evaluate: expression}` is a Grap-library projection convention, not evaluator
syntax. It shows the stored expression, an arrow, and the returned value from
its own read-only `at` occurrence. When wrapped, the arrow stays with the stored
expression and the result is indented underneath. Result children are ordinary
navigation stops and can be copied and folded independently. A result containing
another `evaluate` field can invoke that projection again with an ordinary fresh
evaluation allowance. Ordinary call-shaped values elsewhere remain editable data
until explicitly evaluated.

The presentation library offers an opt-in interpreter for
`{value: source, projection: function}`. Pane views try it only at entry,
following cells through their ordinary definition paths. The document view
leaves the declaration as editable data. It applies the function to the source
as data and projects the result with `at` using the normal
projection. An absent result reveals the stored source using that same normal
projection. Nested declarations remain data, including inside list or record
panes and computed results. Entry intentionally follows cells to the first
non-cell; this entry policy is separate from explicit projection scopes.
The workspace does not interpret this
wrapper. Raw exposes its stored fields in either view.
Explicit `{render: expression}` values evaluate and project their result. They
request an observed evaluation memo at their current view/path; hosts without a
computation runtime evaluate directly. The existing Grap observer tracks cell
definitions (including missing ones) and native observations. Untracked foreign
reads, unrecorded effects, and evaluator halts prevent reuse. Expression and fuel
are explicit inputs. This memo retains only the computed Value, never widgets,
handlers, or rendered output.

The presentation library also offers an opt-in record outline:
`{outline: [field-a, field-b], field-a: ..., field-b: ...}`. The list orders
field references, not copies of their contents. The ordinary `outline:` field
label selects the list and reads its displayed name from the current definition.
The list sits below the label without another indent, giving record and list
selections distinct bounds even without extra fields. Select-all still reaches
the root; deleting the outline field leaves the remaining record in its ordinary
presentation. The list uses a vertical, unbracketed presentation with a local
element partial for the field's name and
disclosure heading. An item-composition callback adds the indented body outside
the heading's cell boundary, so reference selection and library tint cover only
the heading, not the separately projected source field. There is no separate
toggle strip or selected-tab color. The same list-entry combinator
supplies stable element paths and pending insertion for both ordinary bracketed
lists and these columns. Column gaps, including half-gaps at either end, offer
an insertion line on hover; clicking selects an ordinary missing element.
Gaps beside an existing pending and all read-only gaps stay inactive. Empty
columns retain an ordinary empty-list handle.

A heading selects the real list element, so navigation, insertion, deletion,
and picking remain list operations. Removing an entry removes only the reference,
not the source field. The body uses `jump` at `outline / element / field` to the
source record's field, composing with any enclosing conject. Repeated entries
therefore share document edits but have independent selection and fold state.
Section lists use the same unbracketed column with spacing between items.
Computed outlines use `at` for their bodies instead: copy and folding remain
available without inventing a document source.

Visibility uses the existing undoable fold annotation at each body occurrence,
without document edits or a separate state store. Clicking a heading folds its
body and selects the heading, keeping selection out of hidden content. The
outline has no outer braces. Only unlisted fields appear in an ordinary record
footer, omitted when there are no extras or pending field insertion. Its braces
select the whole record; it has no separate fold state or control. A missing
referenced source field gets its ordinary empty picker. A malformed outline
declines to the normal projection.
This is a library presentation available at any record, not a special root or
workspace model. Raw still projects the underlying record normally.

Assigned-size panes instead use `{value: source, viewport: function}`. The editor
recognizes this contract at pane entry and passes the settled logical `width`
and `height` along with the unevaluated `value` as data. An optional `prepare`
function runs first, receiving only `value`; its result replaces that argument
to the viewport function. Preparation uses the ordinary dependency-tracked
application memo at the declaration's occurrence. Its inputs are the callable,
source data, and optional `fuel` (defaulting to Grap's usual allowance), plus
observed definitions. Dimensions are deliberately not inputs. Effects or
untracked reads prevent reuse, just as for other observed Grap evaluations.
This lets a pane construct a program independently of its size without
changing the contract of existing declarations. A preparation absent exposes
the stored source without calling the viewport function.

The viewport result goes through the same `at`
projection and handler machinery as other computed content. There is no
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

Grap's `descend`, `descend path`, `jump`, and `at` forms decode through the
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
return the same measured widgets as other projections, not control opcodes.
Their settled placements run against `HoverPass`; its `HoverOutput` holds the
winner and after-hover continuations. Binding those with `ResolvedHover` produces
paint and handlers independently, using Puri's generic phase composition.
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

Native editing scopes also pass through this Grap adapter. `site path` and
selection paths name displayed occurrences; the host retains the interpretation
used by later document edits. No mutable editor, scope handle, or opaque native
value is encoded into GID. Completion continuations receive a read-only scoped
view, so looking up their committed path reads the newly inserted source value.
Scope construction and arbitrary Grap-defined editor wrappers are not exposed
as Grap functions in this checkpoint.

Drawing programs use scoped foreign operations for fills, strokes, paths,
transforms, and clips. A temporary path builder belongs to that synchronous
evaluation. A visible program records once in the frame; hover and painting
share the recording and its source origins. There is no cross-frame canvas memo.
The installed frame's hover probes retain that recording for subsequent pointer
hit tests; every successor frame still makes its own recording.

## Controls and state

The controls library's `with controls` projection runs a controls lambda with
`control state` and `update` arguments. State is an arbitrary GID value held under
the occurrence's control-state annotation field; a missing field supplies the
explicit `no control state` absent. Defaults belong to Grap code, not to this
container. The controls lambda emits widgets and returns ordinary parameters,
which the view lambda receives unchanged.

The plain `slider` takes `value` and `on change`, with optional numeric bounds.
It returns its displayed value. During input dispatch its Grap change handler
receives `value`, the current `control state`, and the scoped `update` callable.
Calling `update {value: ...}` replaces that control state, preserving unrelated
site annotations. Updates are staged until the handler completes; ordinary
absents retain effects, while decline or an evaluator halt discards them.
The update capability is unavailable while building the controls. The CAM radio
and tree controls still use their existing keyed state adapters; they have not
yet migrated to the plain slider's explicit value/handler interface.

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

Leaf/recursion capabilities include `text`, `slot`, `descend`, `descend path`,
`jump`, and `at`. `descend` takes `step`; `descend path` takes
`steps`; `jump` takes `steps` and `document path`; `at` takes `steps` and `value`.
All paths use the path library, with no opaque editor or scope value in Grap.
Custom native conject functions are currently Rust-side; the Grap-facing jump
provides the standard document-path interpretation.
`canvas` accepts width/ascent/descent and a drawing-program value;
it does not construct a list of draw commands. Its optional fuel argument
configures the later drawing evaluation, as with the stored drawing convention;
omitting it uses the evaluator's ordinary default, not the layout call's remainder.
Row/column gap defaults to zero, column baseline to zero, padding sides to zero,
and text paint to the ink face. Other required inputs are validated. Invalid
builder calls return absents; containers propagate a failed child computation
without emitting that container. A final absent, multiple root emissions, or
evaluator halt discards all output. There is no sticky failure flag overriding
Grap recovery. An intentionally empty layout can emit an empty row or column
with `{}` as its child computation; an empty `do` is not a successful computation.

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
{value: expression, ...}
{closure: {params: [...], body: expression, environment: {...}}}
{ffi: function-cell}
```

Evaluating a lambda captures its lexical environment in a callable value.
Grap-defined calls evaluate every declared argument before the body. Argument
labels are cell identities, so reusing a library parameter cell is meaningful;
its display name does not participate in binding. The ordered parameter list
is the current representation, not a settled general pattern language.

A value wrapper evaluates its `value` expression in the calling environment.
It introduces no lexical binding and does not memoize a result: evaluating the
same wrapper again evaluates its expression again. The name and unrelated
metadata are ignored. Calls and complete lambda forms take precedence when a
record contains several recognized forms. `let`/`where` retain their existing
bind-once semantics and separate `value` field identity; they do not own or
discover wrappers. As with calls, wrappers inside inert data or returned by a
function remain data until explicitly evaluated.

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
functions alike. Other absents remain ordinary results, including a non-callable
result. Effect-only commands, including selection and site-state setters, return
`{}` on success, including when clearing state. Getters return specific absents
when the requested state is missing.

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

Fuel limits individual evaluator runs to help catch accidental runaway programs;
it is not a security boundary. Calls within one evaluator context still spend
that context's allowance. Separate projection evaluations start fresh and may
specify their own budgets; the projection context retains no remaining-fuel
state and does not clamp a nested render's requested budget. In particular,
an endlessly self-reproducing projection can still hang the editor. Preventing
all such loops is not a contract of the current fuel mechanism. Preview meshes,
images, and display errors do not carry unused evaluator fuel through rendering.

## Control functions and absents

The control library's `all {expressions: [...]}` evaluates independent expressions
in the calling environment, in order, and returns the list of results, including
absents. An empty `all` returns `[]`.
Ordinary absents do not suppress later expressions or roll back earlier effects;
evaluator halts still stop execution. Its compact projection uses the same
expression-list presentation as `do`, with normal expression completions.

The absent library's `or default {value, default}` evaluates `value` once and
returns a non-absent unchanged. Only an absent result evaluates `default`, in the
same calling environment. It does not validate the value's type or undo effects;
actual evaluator halts still halt rather than invoking the fallback.

`do` evaluates its expressions in order, returning the first absent unchanged
or the last successful value. It short-circuits that sequence, not the evaluator:
an enclosing `match` can handle its result. Earlier effects are not rolled back.
An empty `do` returns `missing final expression`. Ordinary `let` and `where`
bindings may bind absents without short-circuiting; a structural binding mismatch
returns a pattern-mismatch absent.

The logic library uses open `{bool: true-cell}` / `{bool: false-cell}` records.
The payload is compared by identity, never evaluated or compared by its name.
Its `require` function takes a `condition`, returning `{}` for true, a
condition-not-met absent for false, or a not-boolean absent for another value.
An absent condition propagates unchanged. This is an ordinary guard used in
`do`, not evaluator syntax or a configurable bind mechanism.

The list library's `iterate` and `unfold` finish only when a step returns
`{absent: iteration-finished}`. They then return the previous state, and for
`unfold` the accumulated items as well. Other absents propagate unchanged;
actual evaluator halts still halt. `fold` remains an ordinary fold, allowing
any accumulator value rather than imposing short-circuiting.

The sequence library provides streaming iteration without materializing a list.
A sequence is a zero-argument callable returning `{item, next}`, where `next`
is another zero-argument callable, or `{absent: iteration-finished}`. The yielded
record is open to unrelated fields. Other producer absents propagate unchanged.
Calling the same pure closure again returns the same item; consumers advance by
calling the returned successor, not by mutating a cursor. `range {count}` produces
f64 indices from zero up to (excluding) count, a nonnegative integer no greater
than 2⁵³ so every increment remains exactly representable.
`for each {items, action}` pulls one item and calls `action {item}` before pulling
the next; an absent action result stops it, otherwise completion returns `{}`.
Even an `iteration-finished` absent from the action propagates as a failure;
only a producer's return can mark completion. `collect {items}` explicitly
materializes a GID list. Consumers retain the current callable and obey ordinary
evaluator fuel limits. An empty range is a callable that immediately finishes.
Grap-authored producers can use the same record/callable protocol; there is no
new evaluator syntax, hidden stream handle, or precomputed list of indices.

The [control library](../progred/src/libraries/control.rs) supplies structural matching
and quote/unquote as ordinary Rust functions using that evaluator interface.
`match` evaluates its subject once, then tries ordered cases. Record patterns
are open; list patterns are exact and ordered; `{bind: cell}` captures a value.
Repeated binders require equal captures. Only the selected expression runs,
in the caller's environment extended by captures. Its absent result is final,
not a request to try another case. There is no implicit match-subject binding.

Case lists and `let`/`where` binding lists supplied by another expression are
evaluated once and inspected as runtime containers. Selected expressions and
binding right-hand sides use `Context::eval_runtime_code` in the appropriate
environment; they are not serialized back to GID first. Embedded native closures
therefore retain their original code origins and captures. Patterns still use
the GID-oriented structural matcher, converting only the individual pattern.
Literal clause lists keep their existing prepared-expression path.

`quote` walks the original raw structure, replacing `{unquote: expression}`
leaves by evaluating their expressions in the caller's environment. It never
revisits a spliced result. Outside that traversal, an unquote-shaped record is
ordinary data. There is no evaluator-level quote/literal form.

The normal projection shows a compact quote call with a double-quote prefix,
and an `{unquote: expression}` record with a backtick prefix. The
unquote prefix selects the whole unquote record; its body remains independently
selectable at the stored field path. The expression
uses Grap's local shallow-reference projection, while quoted data keeps its
ordinary projection. Unrelated fields do not block either compact projection;
Raw still exposes them. Active field insertion falls back to the full record/call
display. These markers
are display notation only, not new evaluator or text-bridge syntax.

Semantic failure is an open GID record:

```text
{absent: reason-cell, ...occurrence-details...}
```

The reason cell supplies names and other static metadata. Occurrence fields
can identify a missing cell, invalid value, or cycle. Code inspects identities,
not human-readable diagnostic text. Hosts and tests inspect the returned value;
there is no parallel Rust diagnostics list that can reject a successful result.
There is no unspecified reason or zero-argument absence constructor. Each
producer chooses a reason, reusing library reasons and adding occurrence details
where useful. Failed atomic-editor conversions include the invalid input.

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
value so a pass-through preserves unrelated fields. Public `evaluate` and
`apply` return `Evaluation<RuntimeValue>`. `RuntimeValue::into_value` (or
`Evaluation::into_value`) explicitly materializes GID; the `*_value` entry
points adapt callers that still need that representation.

Expressions own shared lowered code and source locations. A returned closure
keeps its body and lexical environment alive, not its creating evaluator,
host, compiled thunks, or capabilities. Applying it starts with the receiving
evaluation's execution caches, host, and capabilities. Foreign references
retain a cell identity and resolve its implementation in that receiving
evaluation. Code origins survive; a closure's creation-time call stack is not
part of its invocation-time call stack.

Closure GID records also retain an optional `body origin` field beside `body`
and `environment`. It describes the body's source, not the call that created
the closure. The ordinary record forms are:

```text
{document source: path}
{source cell: cell, definition source: document | {library: id}, source path: path}
{input source: path}
```

Paths use the existing path-library convention. The shared codec lives in
`grap::path`; Progred's path library re-exports it and supplies its presentation
names. These are ordinary GID records, not new atoms. The editor anchors source
at the closest enclosing cell definition, or the document root when there is
none. A cell origin does not remember which reference occurrence led to it.
The low-level evaluator's `input source` form remains relative to a
caller-supplied input; editor evaluations supply anchored origins before
retaining closures across runs.

Materializing and reloading a closure preserves its body's known origin,
lexical captures, and the separate origins of embedded closures. Child code
locations extend the body origin. Generated bodies without an origin remain
unattributed; neither materialization nor invocation fabricates one. Missing
or malformed origin metadata does not prevent calling a valid closure. Origins
are best-effort annotations: decoding does not resolve or retain the referenced
document/library, and unavailable locations simply fail to link on hover.
No creation-time stack, projection occurrence, or evaluator identity is encoded.

Native `ForeignFunction::new` callbacks return runtime values, and
`Context::eval` evaluates an argument to a runtime value. Generated runtime
arguments remain values, including closures: they are not materialized and
reinterpreted as expressions. Raw expression inspection and explicit GID
adapters remain available for macros and existing consumers.
`ForeignFunction::from_value` is the explicit adapter for a GID-returning
implementation. Progred now carries runtime values through evaluation results,
presentation, controls, layout descriptions, drawing programs, and retained
slider/event callbacks. Descending into a returned container retains its runtime
children. Existing data-oriented partial projections use an explicit adapter;
it borrows stored GID directly or lazily materializes a shared GID view. The
original runtime value remains available to runtime-aware partials and children.
Copying a result and writing document/UI data are GID boundaries.

Fold classification, collapsed-container display, and source highlighting inspect
runtime atoms/container shape directly rather than asking for a complete GID
view. Text, blob, and color partials likewise inspect only their own facets. A collapsed
value's picking handler retains the runtime value and materializes it only when
invoked. Empty partials use the runtime interface so declining without inspecting
anything does not itself request conversion. Expanded structural lists/records
inspect positions/labels directly and descend into retained runtime children;
stored list positions remain unchanged. Control-form partials and shallow Grap
reference/declaration helpers also inspect runtime inputs directly. General Grap
call/lambda/value/FFI projections, evaluate-form recognition, numeric facets, and
arithmetic notation now do likewise. Call layout inspects labels rather than
argument values; parameter discovery reads native closure parameters directly,
without materializing code or captures. Both `evaluate` and presentation's
`render` pass runtime syntax into evaluation, including memoized evaluation.
The memo input uses the same conservative runtime equality as results, so
closures with different origins cannot silently reuse one another. Those
origins now differ in their GID representations too. Domain-specific and other
legacy partials still use the GID adapter.
Opening the writable color picker or
starting a numeric scrub requests GID for document-editing callbacks; simply
recognizing or displaying those facets does not materialize the enclosing record.

Scoped layout programs retain runtime results through nested child collection,
absence propagation, and the final output check. An absent's detail fields may
themselves contain retained callbacks. The `drawing` constructor wraps its
runtime configuration unchanged; layout picking retains its payload until
activation writes it into editor state. Border-projection composition captures
its runtime callable in the wrapper's lexical environment and retains the
projected result inside its border/`at` description. Declaration projection also
passes its callable and argument as runtime values. Neither boundary needs to
materialize callback code or captures. Narrow path/paint readers still use GID
adapters.

Generated drawing syntax may contain retained native closures. Explicitly
interpreting that syntax lowers its containers without serializing the embedded
closures. The expression-facing host application adapter preserves the existing
contract: Grap parameters bind supplied data, while native functions receive
argument syntax to interpret. This is distinct from `apply`, whose callable and
arguments are already evaluated.

Toolpath execution retains runtime programs through nested `sequence paths`,
point/axis mapper scopes, and tool-scope results. Mapper inputs and point results
use runtime numeric fields; absent details retain runtime children too. The 2D
preview constructor and projection pass the program through without materializing
it. Tool-profile parsing remains an explicit GID boundary. Tree collection,
mapping, memo inputs/results, and the tree-program cursor's control/view handoff
also retain runtime values. Tree memo comparisons include callable code/capture
identity and the separate source-linked hierarchy, not just serialized GID.
The 3D preview constructors, projections, and recording memo retain runtime
programs and results as well. Model, playback, color, and tool-profile decoders
still use GID views of their own data. Recorded failures adapt to GID at the
existing render-outcome boundary; worker requests contain geometry, never
runtime closures. Fidget and cutter projections check their identifying fields
before requesting a GID view, so unrelated executable containers aren't
materialized merely to decline a projection.
Widget preparation also receives the runtime value, not an eagerly materialized
GID copy. A selectable widget converts only when the user actually picks its
value into the editable document.

Inline code retained by the editor is anchored to its original document
occurrence before evaluation; cell code retains its definition source and path.
Later invocation does not rebase those locations onto the drawing program.
Cmd-hover chooses the innermost call with an available projected occurrence.
Memo result comparison still conservatively includes native closure code/capture
identity. It can decline reuse for separately allocated but equivalent closures;
this is a cheap reuse test, not Grap value equality. GID equality now includes
the explicit origin metadata instead of silently discarding it.

Materialization preserves existing sharing of runtime records, lists, and
captured environments within one returned value. Its address table lives only
for that conversion and does not intern equal values. A runtime value may retain
a lazily materialized, shared GID view for data-oriented consumers; this is a
representation conversion, not a computation memo. Environments materialize their effective bindings, newest first,
without converting shadowed bindings. Closures still capture the shared lexical
environment, not a statically computed subset of free variables. Native
constructors that just assemble evaluated arguments should use `Context::eval`
and `RuntimeValue` containers, leaving conversion to a real GID boundary.

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
