# Grap and projections

Grap is embedded in ordinary GID values. Its evaluator, library conventions,
projection composition, and editor adaptation have separate responsibilities.
Earlier rationale and proposed language directions are in
[historical notes](history/projections-notes.md).

## Packages and libraries

The [Grap runtime](../grap/src/lib.rs) depends only on GID. It implements
lambda/application, value wrappers, environments, callable representations, fuel, source
origins, and stable evaluator absence reasons. It has no editor, geometry,
window, or file services.

Progred's [libraries module](../progred/src/libraries.rs) contains
conceptual libraries as modules alongside their editor-facing widgets, each a
contribution through the [library interface](../progred/src/library.rs): names,
text, f64, Grap, presentation, layout, and other domains. The built-in
`libraries::grap` module owns Grap's self-description, the `evaluate` Rust
function, its evaluation control and arithmetic, and Grap's projection; CAD
concepts remain separate libraries. Nothing in this representation requires Rust FFIs, which leaves room
for [editor-authored libraries](deferred.md#editor-authored-libraries). A
`Library` carries definitions, one partial projection, and the projections its
cells provide: a kind's for the records that say they're of it, a function's for
calls of it. A library with several forms composes them with
`compose_partials`: first success wins, and the empty composition declines. The
editor composes these library projections in load order above its one total
structural fallback, after the projections kinds provide. `Libraries` keeps an insertion-ordered unique map keyed by
stable random library identities supplied externally; the identity is not a
field inside the library. Loaded libraries are read-only. Repeating a library
identity replaces its entire contribution in place before composition. Loading
is an explicit editor operation, not an FFI or reserved-cell discovery.

Each library stores one sorted definition table. An entry is either an ordinary
`Value` or a shared native definition containing its descriptive `Value` and Rust
implementation. Reading uses the description; calling uses the implementation.
The built-in builders join their data and function declarations once, when
constructing this table; they are not separate lookup registries. A repeated
definition replaces the prior entry. A native definition's description says
it's a foreign function implemented under its own cell; an unnamed one says
only that. A native implementation declares the arguments it
requires, and its description lists them under `params`, as a lambda's does, so
calls to either open their first argument and show slots for missing ones.
Arguments with defaults stay undeclared. The library's own name record is an ordinary
definition under its library identity, so references and name lookup need no
metadata side channel.

[`stack::load`](../progred/src/stack.rs) retains those boundaries and composes
the partial projections and contextual completion providers. A completion provider
receives the query, field/value kind, scope (`Everything`, `Literal` or `Start`),
the hole's path from where its location starts, read-only path and cell
lookups from that same start, lazy enumeration of defined cells, the document's
names for cells, and the loaded libraries' provider.
Library providers compose in library order;
a projection may supply a local vocabulary on its completion control instead.
An empty document root first asks providers with `Start` what a document can
start as; an area leaves out the providers of libraries switched off there. A Grap function stored in a document is
ordinary reachable data; configuration is never discovered through a reserved
cell address or other data outside ordinary root reachability.

A document declares its own libraries under the root's `libraries` field, which
the editor library names beside `row` and `column`, and the editor reads it
only at the root. Each entry, or the cell it names, is a record of the `keys` the library
owns and the `projection` that draws a record carrying any of them: a function
of `value`, the same convention a pane's projection uses. One built-in library,
"document libraries", loaded after all the others, tries the declared libraries
in order on records carrying their keys, so a document library can't take over
another library's values, the root, or its own definition, and switching it off
in an area switches them all off. Records in its own code that
carry its keys, like the patterns its view destructures, are still offered to it.
The projection runs with the layout functions available: a layout it emits draws
the record in place, so whatever it descends into stays editable; a value it
returns is drawn with the area's projections; an absent passes the record on, so a
view that can't draw a value returns an absent before emitting anything (`do`
stops at its first absent). Only a field being added follows
an emitted layout, as a trailing record. A library steps aside where it is already
drawing, or for a value it is already drawing, so a view can show its record's
ordinary projection inside itself, in place or as its result, and nested
document-library drawings stop at a fixed depth. Declarations are read
while projecting, so editing a library's view redraws its records at once. Document
libraries contribute no completions or foreign functions, and other documents
can't load them.

Library labels are once-minted random `CellId`s. Their readable names are
ordinary GID facts, not identifiers derived from names. Text is an open
`{utf8: blob}` convention; f64 is an open `{f64: eight-byte-blob}` convention.
A PNG image is an open `{png: blob}` convention holding the encoded file. Its
projection decodes it each frame and draws the picture at one logical unit per
pixel, shrunk to fit 480 units on its longer side. Bytes that don't decode, or
whose RGBA pixels would exceed 64 MiB, decline to the ordinary record view.
`png width` and `png height` read only the header.
Additional fields do not invalidate a recognized facet, so a line projection
still recognizes text or f64 and a more specific wrapper (for example a unit
around f64) can defer to that facet.

## Projection composition

Explicit Rust memo boundaries can run whole Grap evaluations through the
[dependency-tracking adapter](incremental.md#grap-and-foreign-calls).
Ordinary projection/evaluation remains uncached. Foreign functions opt into
being cacheable; other calls and unrecorded effects prevent reuse.

Normal display uses one composition of ordered partial functions, followed
by a total structural fallback. A partial checks its preconditions and fails
closed by declining; malformed shapes must remain accessible through a later
projection or Raw. Raw is every library switched off: the structural fallback
alone, in the same `Layout` language, with no library's offers or names. It
declines only at a missing location, draws a cell as its definition between
parentheses, and shows labels as short ids. Raw and Plain are sets of
libraries an area shows, as any other set is: Raw shows none, and Plain shows
the name, isa, text, absent and number libraries, so those read as
themselves and the rest as Raw draws it. Arithmetic is Grap's, not the number
libraries', so in Plain it stays the call records it is.
One list in stack.rs defines Plain for the editor and the tutorial page alike.
The Projections menu leads with All, Plain and Raw, then a checkbox for each
library an area can switch off, one bringing a projection or offers; a set
matching no preset is a custom one. Choosing the preset an area already shows
goes back to every library. Specific
domain projections precede general ones.

A record that says what kind of thing it is draws first as its kind provides,
ahead of every library's partial: the stack's first partial reads the record's
`isa` and applies the projection its libraries registered under that kind's cell,
composed in load order, or declines where the kind provides none or the record
says no kind, and the libraries' partials are tried in load order. Projection
inputs carry the provided projections, so a projection can ask another cell for
its own: Grap's call kind draws a call as its function provides, else as a
call. Grap's calls, lambdas, closures and value wrappers provide theirs;
control's `match`, `let`, `where`, `do`, `all` and `quote` and the infix
operators of Grap's numbers provide theirs for calls of them, and the unquote
kind for unquotes; each of Fidget's node kinds provides its own. An area that
switches a library off leaves its cells' projections off, so with Grap off an
operator call is a record again.
Measured on 22 October 2026, this took the IoP source form's projection scope
from 2.44 ms to 2.23 ms a frame and its allocations from about 35,000 to 30,700;
the frame canaries moved within their noise. The current host partials are Rust
callbacks, while presentation declarations can apply ordinary Grap callables.

Recognition checks the fields a partial uses, not the absence of unrelated
fields. A compact form shows the fields it doesn't draw after it, as a trailing
record, so it hides nothing: `structure::record_form` takes the fields the form
draws, handing their values to the drawing in order, and appends the rest. Missing or malformed required contents decline normally,
without reserving the record or blocking later partials. A convention whose
tags are mutually exclusive settles a record carrying several of them its own
way: layout decoding takes the first tag in a fixed precedence and ignores the
rest. Records are identified by required positive
evidence and stay open unless a domain explicitly defines a closed shape.
Active field insertion can use the general presentation to keep its picker
visible.

[`ProjectionInput`](../progred/src/display.rs) supplies the environment, value,
scale, writeability, local selection/annotation data, pending state, and a
lazily built `Site` for its occurrence. Its `default_projection` is one composed
partial function, passed explicitly through recursion rather than buried in
display context. A
partial returns a `Layout<World, Hover>` program that calls the layout builder
with box operations, Puri leaves, and opaque widget/preparation functions.
`Layout` is a reusable program over the object-safe `Builder` interface, not an
enum. The production builder prepares one choice graph; after alternative
selection that graph invokes placement callbacks directly. `Measured` is only an
extent plus a one-shot placement function, never a second container tree.
Structural recording is a separate interpreter used only by tests. There is no
parallel Progred drawing language.

A projection draws three kinds of step: `descend` into what's stored, `at` a
computed value, and `jump` to another location. The root is seeded with the
editor's projection stack, and each projection hands it down; a walk can change
it on the way. Each walk takes two optional projections: `overriding` draws the
value the walk reaches, and `default` is handed down to draw what's inside that
value. Where either is none, the inherited projection stands, as the engine has
it: a pane's entry, which lets its value present itself, included. A projection
reads the one it inherits as its input's `default_projection` (the stack without
that entry), so it builds a change in the open: `first(input, p)` tries `p`
before the inherited one, and `p` alone replaces it. Most walks override only
the value they reach; a pattern sets both to its binders' projection, so the
binders of patterns nested in it are drawn as binders too. A walk's new
default applies below the value, not to the value itself. The total structural
fallback still handles a declined result. These are ordinary preparation
functions, not path-bearing layout opcodes.

`descend` follows steps of the displayed structure without projecting
intermediate containers. It preserves the current location interpretation,
except that following a cell starts a location at its definition.
`jump(steps, location, overriding, default)` extends the occurrence path by `steps` and
a jump step to a location, which it reads from: a document path from the root,
or one within a cell's definition, which keeps that location's start. Its
descendants' paths continue from that jump, for both projection reads and later
edits; no inverse is required.

`at(steps, value, …)` instead projects the supplied value with no document source,
extending the path by `steps` and a computed step, which gives no location,
including for ordinary descendants. A cell followed
inside it still starts a location at its definition, which is stored data, and
an explicit nested jump can establish a source. Existing stored-child partials
use `descend`, not `at` with a copy of the source value. Computed results
use the same `at` combinator, with a `Key(presentation::RESULT)` occurrence
step distinguishing the result from its producing expression. This is a
projection path, not a fabricated field or writable document location.

The composed partial interface receives `Option<&RuntimeValue>`; `partial`
adapts GID-oriented projections to `Option<&Value>`, while `runtime_partial`
retains runtime children and callbacks. `None` means a missing location, not a GID
absent or an empty string. `descend` offers the resolved value or its absence to
the chosen partial; if it declines, the fallback renders structure for `Some`
or the standard empty picker for `None`. There is no separate `missing` parameter
and no fabricated value. A missing value at a real source remains writable;
having no source is distinct and does not offer document editing. Selection,
navigation, and annotations always belong to the displayed occurrence.

The common preparation boundary for `descend`, `at`, and `jump` supplies
selected-value copy/cut handlers below the projected widget's own handlers.
Copy captures the actual projected value, not a document lookup. A projection
that folds supplies its own fold keys, with the default it chose, and updates
only occurrence-local annotations; even an occurrence with no document source
can copy and fold. Only the deletion half of cut requires a document
destination.

The shared read-only decoration uses the palette's `readonly_ground` for computed
output and external definitions. Preparation threads the enclosing ground through
projection composition, including `at`, so nested results do not darken it again.
Writable jumps restore the paper ground. Decoration and edit authority stay
separate: the occurrence's source supplies permissions, and a reference to a
read-only definition is still replaceable at its writable occurrence.

The lambda-name partial shows `λ` when the name is missing and unselected.
Activation selects that missing location for entry. Once selected, it draws the
name library's hole, which offers what's typed as the name; only committing
creates the field. Neither pointer activation nor navigation supplies picker
state. The picker uses defaults for an ordinary selection, including an empty payload.
The name library supplies text suggestions at name fields. Existing names
use unquoted line editors, like parameter names. An explicitly empty name stays
an empty editor, distinct from the missing name's `λ` marker. Ordinary string
projections retain their quotes.
Library completion providers compose separately, so multiple libraries
can contribute offers to one picker.

Grap's expression slots offer a `new lambda` value completion (aliases `lambda`
and `λ`). It inserts `{isa: lambda, params: []}` and
opens its first parameter, whose slot offers a new one; Tab goes on to the
missing body.
The lambda projection draws that unfinished lambda and descends into the body
to show its ordinary empty picker; neither a body nor a name is fabricated.
Beside it Grap offers `new call`, which inserts `{isa: call}` and opens its
`function` field. It's a call before its function is picked, and stays one if
it's left unfinished or its function is deleted, so the call projection draws
its function slot open, an expression slot like any other: a binding, a
lambda, or another call. A call drawn in a form of
its own (an operator's, `let`'s, `match`'s) draws its function slot the same
way, so typing over its function offers the others. There the functions in
reach are offered as themselves, and picking one opens the call's first
argument, as picking its call whole does elsewhere. Once the function is
callable (a lambda needs its body), its parameters show as the call's argument
slots, which Tab visits next.
An empty parameter slot offers only a new parameter, a new document cell: for
whatever is typed, `x — new parameter`, named `x`; with nothing typed, `new
parameter`, which has no name yet, and whose name is picked next.

A parameter with no name of its own, or an empty one, is called what its list
calls it: in order, the first of a, b, c… that none of the list's named
parameters uses. The name is computed wherever it's shown and never stored, and
it's drawn grey: in the declaration as the placeholder in its name's editor,
which typing replaces, in references to the parameter, and in offers. A
declaration draws its name with the same line editor whether it has one or not,
so a missing name and an empty one look alike. A line drawn where its value is missing is
that place's hole too, a Tab stop with no offers: its first edit writes the
value, and the selection stays on it at its edge, typing going on in its line
from the same caret, the write and the rest of the typing one undo step; Enter
with nothing typed writes the empty string, which names nothing. A line's
placeholder (`LineEdit::placeholder`) shows dim where its text goes while
there's none, the caret before it; with nothing to show and no placeholder, a
line shows an empty frame. A
reference finds its list by walking out from where it's stored, the same walk a
slot's offers make to find what's in scope, so each list names its own; an
inner lambda's first unnamed parameter is `a` too. In a computed value, such as
a closure in a result, the walk goes out through that value instead, which the
projection keeps for what it draws inside (`Env::read_computed`), so the
closure's lambda reads as it does in its source. A closure's environment holds
parameters of the lambda that made it without the list that named them, so it
calls its own: its bindings with no name of their own are e₀, e₁, e₂…, the
numeral a subscript, in its order, skipping what its named ones use, grey in
its labels and in its lambda's references to them. A computed name is a `Name`,
so it can carry subscripts, and an offer shows one as it's drawn
(`CompletionText::Named`). Anywhere else, an unnamed parameter shows its short ID.

The expression-use partial also handles missing values with a focused completion
provider. It lists statically visible bindings, Grap's new lambda and new call,
the functions in reach as calls, the other libraries' `Everything` offers, Raw's
constructors, quote, and what's typed read as a value.
Only explicit expression children use this provider, not quoted data, parameter
declarations, or ordinary structural containers. Provider composition is lazy:
only an active picker reads the loaded libraries' provider, from the toolkit.
At a missing expression, the default partial gets the first opportunity to
present selection-driven interactions (such as the new-color picker), before
falling back to expression completion.

The shared `structure::list(Some(child_projection))` combinator explicitly
applies a partial at each immediate element. Standard lists use the same
renderer with `None`. Lambda parameters, match cases, let/where bindings, and
do expressions each supply their element projection this way.
A record's fields read through one `structure::Fields` spec: which come first,
which show as slots to fill even where the record lacks them (a call's
parameters), how a field's name reads (a closure's unnamed bindings), and a
partial for a field's value. A field being added is drawn among them, where it
will stay. The spec draws a whole record (`Fields::record`), or the fields after
what a projection draws of the record itself (`Fields::form`, `Fields::after`):
a call's arguments after its function, a form's other fields after the form.
Standard records use it with nothing set. Both share `record_braces` geometry.
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
element position. Separators adopt their container's offered width before both
interaction and painting: a column's width, or their intrinsic width in a row.
Each call returns
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
metadata, and quoted data use the normal deep structural fallback, and local
use-site overrides do not leak into them. `descend` preserves the child's
stored path while selecting its projection functions; Grap display uses this
explicit composition rather than teaching the raw structural fallback about
Grap. In lambda parameters, direct
`let`/`where` binders, and pattern binders, a cell whose definition contains
a text name projects as `(name)`: the usual cell parentheses surround an
unquoted line editor at the real `Follow` → `name` path. The cell and its
definition are one thing: the walk to the definition is a `part`, so the
definition is no stop of its own and arrows go from the cell to its name.
Backspace empties the name, then removes the name field, the parameter
unnamed but still defined; once more lands on the parameter, and again removes
it. A cell with no definition draws as a hole whose one offer is the name
typed, which defines it. This contextual projection is not in the default
stack. The definition's other fields, and one being added, follow the name as
a trailing record. A name that isn't text declines to the ordinary projection;
a missing one is its line editor with nothing in it.
Ordinary cells and quoted data are unchanged; named use sites remain shallow
references.
Calls use a stored or inline
lambda's declared parameter order when available, then the ordinary order for
extra fields. Missing declared arguments appear as ordinary editable empty slots
at their real field paths; they remain absent from the document until edited.
An active missing argument stays in its declared position rather than gaining a
second trailing row. This is a raw definition lookup, not evaluation of the
callable: a native function named by its cell uses the `params` its description
lists, and computed callables use the ordinary order.
A number operation's name carries the same representation subscript used by
literals: it is a formatted name, `["+", {subscript: <the f64 cell>}]`, so a
call's function reference and the picker's offer both draw it from the
definition, and renaming f64 renames the subscript. The label still selects
the call's `function` field. A comparison's subscript identifies its operand
representation, not its boolean result. Fidget's arithmetic is named the same
way, with fidget's tag.
Numeric facets explicitly include an optional stored name beside the number,
separated by spacing rather than binding or record-field punctuation. The shared
name decorator descends to the real `name` field with an unquoted text editor;
the number retains its own editor and scrub target. The numeric target includes
the representation subscript: clicking it selects the number, and modified
dragging scrubs just as over the digits. It does not include the name. While
the command modifier is held, a writable number's digits are outlined as a
hovered value is, in the number's own color rather than the selection's, and
the pointer over it is a left-right arrow, which a drag keeps until release.
This is
opt-in library composition, not automatic merging of every matching facet's projection.
Normal record order is named fields alphabetically by display name, with cell
identity breaking ties, then unnamed fields by identity. With the name library
off, Raw among those, every field is unnamed, so order is by identity.

Grap's `{name: text, value: expression}` wrapper projects as `name = expression`.
The name slot is always present: a missing name uses Raw's ordinary empty
picker, where what's typed reads as text. The expression uses the local
shallow-reference projection. Other metadata does not block recognition and
follows as a trailing record, and both editors retain their stored field paths. This is a named expression, not a
`let` binding; the evaluator ignores its name. Named numeric facets remain
independent of this Grap-specific wrapper.

Within Grap's library, calls, lambdas, and value wrappers are tried in evaluator
precedence order. Each remains an ordinary partial: malformed contents decline
just like a missing required field, allowing the next partial to try. Recognition
does not reserve a record or force structural fallback.

A Fidget field is Fidget's data: each node says its kind (`{isa: subtract, left,
right}`, `{isa: sqrt, operand}`, `{isa: axis, axis: x}`, `{isa: translate,
field, x, y, z}`) with Fidget's own kind cells and operand fields, and each kind
provides its projection. The Fidget library projects field arithmetic as infix
expressions, other scalar operations as named argument groups, and coordinates
as shallow references.
Grouping preserves the expression tree, including right-nested operations of
equal precedence; it never reassociates floating-point arithmetic. Names come
from ordinary definitions. Operands retain their stored paths and stock editing
controls, while an operator targets its whole expression. Named fields expose
their editable name beside the formula. Unrelated fields on an operation or its
operand record do not block notation or change grouping. Incomplete forms
decline; translation keeps its explicit named parameters, each offset none when
left out. Fidget has no Grap functions for its nodes: a field is written as
data, and Grap assembles one procedurally with `quote`, unquoting what it
computes. Its previews remain Grap functions, the viewports presentation calls. These are source projections only, independent of the opt-in
rendered viewport.

Grap calls of a number type's arithmetic and comparison read infix as well, the
operator tagged with its representation. f64, f32 and u64
are only numbers: how each is stored, read, spelled and drawn. Their
operations, the Rust functions that run them and the infix drawing are the
Grap library's ([`grap/numbers.rs`](../progred/src/libraries/grap/numbers.rs)),
which loads before them so that a number it draws takes its operators as
typing. They do from the
moment the operator is chosen: a missing operand is an empty slot whose picker
opens in place, and other fields, including one being added, follow in a
record. A line too narrow for the expression wraps before its operator.

An evaluation, `{isa: evaluation, expression}`, is a Grap-library kind whose
projection shows an expression's result, not evaluator syntax or a field hook
in the structural walk. The evaluator never observes it; a host that never
loads the projection never sees it. It shows the stored expression at its real
`…/expression` path, a dim `→` that selects the whole record, and the returned value from its own read-only `at` occurrence, all in
one group. When wrapped, the arrow stays with the stored
expression and the result is indented underneath. Result children are ordinary
navigation stops and can be copied and folded independently. An absent result
projects like any other computed value. Recognition is open like text and f64:
other fields do not block it and follow as a trailing record, and an earlier
partial in the composition wins if several match. Raw shows only the stored
record.

A closure, as a result shows one, reads as the record it is, its fields in a
fixed order: its lambda, drawn as a lambda is, then the environment it closes
over, then its body origin, shut until it's opened, then any others. A result containing
another `evaluate` field can invoke that projection again with an ordinary fresh
evaluation allowance. Ordinary call-shaped values elsewhere, including returned
ones, remain editable data until explicitly evaluated.

The separate `grap` field is ordinary library vocabulary saying its contents
belong to the Grap domain. It requests no evaluation. The Grap library offers a
root template (a document whose `grap` names a new program rendered in a pane),
and draws the program `grap` names as an expression slot: the cell keeps its
parentheses, and an empty program offers Grap's suggestions rather than the
whole vocabulary.

Grap's other root template is a notebook, `{isa: notebook, entries}`, whose
entries are cells. The record and its list are one thing, the notebook, marked by a
bar down its side that selects it; its entries are stops inside, numbered in
its margin, a number selecting its entry. Each entry draws as its cell's
expression, an arrow, and what it evaluates to, beside it or below it where
there's no room. An entry whose cell has no definition yet draws its expression
and arrow, with no result, so a new entry shows no error.
Each entry evaluates at its own occurrence, so its result is memoized like a
pane's and recomputes when a cell it reads changes, and an error in it opens to
its callers. Entries refer to one another the standard ways: Cmd-clicking,
copying, or naming. The entries are drawn as a list, with a list's editing:
clicking between two inserts a place for one there, and deleting one leaves its
place to fill. Such a place offers "new entry", a fresh cell whose expression
starts from what was typed. Enter on an entry, once its own handlers decline
it, starts a new entry after it, or with Shift before it, going straight to its
expression. An empty notebook draws a "new entry" that starts its first. An
element that isn't a cell draws as plain data. The arrow selects the entry
itself rather than its expression.

The presentation library offers an opt-in interpreter for a projected value,
`{isa: projected value, value: source, projection: function}`. Pane views try it
only at entry, as its kind says,
following cells through their ordinary definition paths. The document view
leaves the declaration as editable data. It applies the function to the source
as data and projects the result with `at` using the normal
projection. An absent result reveals the stored source using that same normal
projection. Nested declarations remain data, including inside list or record
panes and computed results. Entry intentionally follows cells to the first
non-cell; this entry policy is separate from explicit projection scopes.
The workspace does not interpret this
wrapper. Raw exposes its stored fields in either view.
A render, `{isa: render, expression, fuel?}`, is the other thing a pane's entry
presents: it evaluates its expression, with its own fuel when it gives one, and
projects the result. Like a declaration, it isn't part of the ordinary
projection, so anywhere but a pane's entry it's the record it is, its
expression editable; a tutorial slot, a pane of its own, presents one too.
A render requests an observed evaluation memo at its current view/path; hosts without a
computation runtime evaluate directly. The existing Grap observer tracks cell
definitions (including missing ones) and native observations. Uncacheable foreign
calls, unrecorded effects, and evaluator halts prevent reuse. Expression and fuel
are explicit inputs. This memo retains only the computed Value, never widgets,
handlers, or rendered output.

The presentation library also offers an opt-in record outline:
`{isa: outline, sections: [field-a, field-b], field-a: ..., field-b: ...}`,
drawn as its kind provides. The list orders field references, not copies of
their contents. The ordinary `sections:` field label selects the list and reads
its displayed name from the current definition.
The list sits below the label without another indent, giving record and list
selections distinct bounds even without extra fields. Select-all still reaches
the root; deleting the `isa` field leaves the remaining record in its ordinary
presentation. The list uses a vertical, unbracketed presentation with a local
element partial for the field's name and
disclosure heading. An item-composition callback adds the indented body outside
the heading's cell boundary, so reference selection and library tint cover only
the heading, not the separately projected source field. The same list-entry combinator
supplies stable element paths and pending insertion for both ordinary bracketed
lists and these columns. Column gaps, including half-gaps at either end, offer
an insertion line on hover; clicking selects an ordinary missing element.
Gaps beside an existing pending and all read-only gaps stay inactive. Empty
columns retain an ordinary empty-list handle.

A heading selects the real list element, so navigation, insertion, deletion,
and picking remain list operations. Removing an entry removes only the reference,
not the source field. The body jumps at `sections / element / field` to where the
source record's field is stored, whatever jumps led to the outline. Repeated entries
therefore share document edits but have independent selection and fold state.
Section lists use the same unbracketed column with spacing between items.
A computed outline is stored nowhere, so its bodies are drawn with `at`
instead: copy and folding remain available without inventing a document
source.

Visibility uses the existing undoable fold annotation at each body occurrence,
without document edits or a separate state store. Clicking a heading folds its
body and selects the heading, keeping selection out of hidden content. The
outline has no outer braces. Only unlisted fields, other than `isa`, appear in
an ordinary record footer, omitted when there are no extras or pending field insertion. Its braces
select the whole record; it has no separate fold state or control. A missing
referenced source field gets its ordinary empty picker. A malformed outline
declines to the normal projection.
This is a library presentation available at any record, not a special root or
workspace model. Raw still projects the underlying record normally.

Assigned-size panes instead use a sized value, `{isa: sized value, value:
source, viewport: function}`. The editor recognizes this kind at pane entry and passes the settled logical `width`
and `height` along with the unevaluated `value` as data. An optional `prepare`
function runs first, receiving only `value`; its result replaces that argument
to the viewport function. Preparation uses the ordinary dependency-tracked
application memo at the declaration's occurrence. Its inputs are the callable,
source data, and optional `fuel` (defaulting to Grap's usual allowance), plus
observed definitions. Dimensions are deliberately not inputs. Effects or
uncacheable calls prevent reuse, just as for other observed Grap evaluations.
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

The [projection runtime](../progred/src/projection.rs) resolves document
locations and supplies borrowed widget inputs. The
[box interpreter](../progred/src/display/measure.rs) composes measurements without
interpreting document traversal or control requests. A value no projection
accepts falls back to [Raw](../progred/src/display/raw.rs), which draws it by its
GID kind, and the layout library keeps its
[Grap event adaptation](../progred/src/libraries/layout/events.rs). Holes are the
toolkit's [picker](../progred/src/display/widget/picker.rs). The layout library
[records its drawing programs](../progred/src/libraries/layout/drawing.rs) into
the toolkit's [recorded-drawing leaf](../progred/src/display/widget/recorded.rs).
The [native completion card](../progred/src/display/widget/completion.rs) owns its
row interaction, keyboard navigation, scroll composition, and placement against
its query. The picker only supplies it offers, query state, and document
callbacks.
The generic [layout choice engine](../ui/measured/src/choices.rs) belongs to
`measured`, independently of those editor adaptations.

Grap's `descend`, `descend path`, `jump`, and `at` forms decode through the
[path convention](../progred/src/libraries/editor/path.rs), also used by site and
selection capabilities. Field keys, list positions, and definition follows have
one encoder/decoder. They produce ordinary preparation functions
using an explicit projection scope; layout has no parallel path vocabulary or
traversal opcodes. Preparation interleaves descendant projection and measurement
before choices resolve. Shared children prepare once per frame, and only chosen
placement continuations contribute interaction and ink.

The stock [line widget](../progred/src/display/widget/line.rs),
[delimiters](../progred/src/display/widget/delimiter.rs), borders, and the
[interaction functions](../progred/src/display/widget/interaction.rs) for
activation and picking are ordinary native widgets and decorators, built on
`widget::before` and `widget::after`; [Puri and the editor
frame](puri.md#widgets-and-hover-continuations) describes how their placements
run against `HoverPass` and bind into paint and handlers. Text and number
projections supply the line widget's spelling, affixes, and conversion callback;
`Layout::widget` carries the resulting measurement program without inspecting
its props. Line conversions are native functions, applied by editing's
`edit_line`; they do not round-trip through Grap.
Completion uses an ordinary native widget factory with explicit kind/provider
inputs. The app adapter constructs document-specific offers and pending
state; the reusable card owns row ink, navigation, and scrolling. Drawing-program
widgets directly use the app's evaluation/source attribution helpers. Both
return the same measured widgets as other projections, not control opcodes.

Grap layout constructors decode to these same functions. The current site's
value and the host's pick capability are separate inputs, so an explicit pick
target need not equal the value being projected.

Grap's `on_event` adapter belongs to the layout library and builds an ordinary
`before` decorator too. One handler decides when
a drawing's handler gets an event, and calls the supplied site-scoped
interpreter with the event as [`site::event`](../progred/src/site/event.rs)
encodes it. The core owns that record and its vocabulary, which the layout
library re-exports, and the app owns staging and committing effects. Native
widgets bypass that interpretation and use their callbacks directly.

Scrolling uses an ordinary native placement handler too. Fidget's handler
closes over a requested annotation-write capability and explicitly applies its
camera update; layout does not interpret a state-scroll result. Acceptance and
the unused displacement use Puri's `ScrollOutcome`, independently of writes.
Document scrolling and widget scrolling share the same conversion of pixel,
line, and page input and its remainder. The conversion knows no document state.

Callbacks become Puri handlers and receive mutable world state at dispatch; no
projection-action enum or central reducer sits between a callback and its
operation. Generic Grap
event handlers receive GID event values. [`site`](../progred/src/site.rs)
creates temporary selection/annotation capabilities in the current document
and view. `site path` exposes the actual projection path through the
[path convention](../progred/src/libraries/editor/path.rs); the local selection getter needs no
address, while the setter accepts one explicitly. Writes are staged and
committed unless the handler explicitly declines or evaluation halts. An ordinary
absent result is still a completed result. Completion continuations
use the same effect interpreter, staged together with the document insertion.

Native editing scopes also pass through this Grap adapter. `site path` and
selection paths name displayed occurrences; the host retains the interpretation
used by later document edits. No mutable editor, scope handle, or opaque native
value is encoded into GID. Completion continuations receive a read-only scoped
view, so looking up their committed path reads the newly inserted source value.
Grap code cannot construct scopes or wrap editor operations.

Drawing programs use scoped foreign operations: `path`, `move to`, `line to` and
`close` build a path, and `fill` paints a given shape, or else that path, under an
optional transform. A temporary path builder belongs to that synchronous
evaluation. A visible program records once in the frame; hover and painting
share the recording and its source origins. There is no cross-frame canvas memo.
The installed frame's hover probes retain that recording for subsequent pointer
hit tests; every successor frame still makes its own recording.
A fill's shape and transform are evaluated like any other argument and read
as ordinary values. Coordinates computed by the program are written with
`quote` and `unquote`, as scenes are; a shape of literal numbers is already a
value.

## Controls and state

The projection of a view with controls, which `with controls` returns, runs a
controls lambda with `control state` and `update` arguments. State is an
arbitrary GID value held under
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
The update capability is unavailable while building the controls. `radio` and
the tree controls instead take a `key` naming their entry in that state and store
their values there themselves; [one control-state
model](deferred.md#one-control-state-model) would move them to the slider's
interface.

## Scoped layout programs

The layout library's `layout program` function takes a raw `expression` and
returns `{isa: layout program, program: closure}`. The closure is an ordinary
Grap closure with its lexical environment, not an opaque native value. The
library's projection for that kind runs the closure with a borrowed
`ForeignOverlay` capturing a Rust layout output buffer; a record of the kind
can carry the data its program descends into beside it. It adds no evaluator
types or syntax.

Within that scope, `row`, `col`, `overlay`, and `alternatives` take a raw
`children` expression. They evaluate it once into a fresh child buffer, then
emit the corresponding native layout. `do`, ordinary function calls, and loops
can produce that sequence; a list of expressions remains inert unless a control
function evaluates it. A row aligns its children's baselines; given a
`baseline`, it aligns their tops instead and keeps that child's baseline, as a
column does. `pad`, `center`, `border`, `bracket`, and interaction wrappers
take a raw `child` expression that must emit exactly one child. `selectable` makes its
child select the value being drawn: it claims the hover there, and a click selects
that value; whatever the child descends into keeps its own target. No buffer borrow is held
while evaluating a body. Successful operations return the ordinary empty record;
their useful output remains in Rust.

Leaf/recursion capabilities include `text`, `slot`, `rule`, `descend`, `descend path`,
`jump`, and `at`. `center` and `rule` act at placement, using the width a
container offers its child, which a column sets to its widest: a fraction is a
column of a centered numerator, a rule, and a centered denominator, with no
measurement in Grap. `descend` takes `step`; `descend path` takes
`steps`; `jump` takes `steps` and `document path`; `at` takes `steps` and `value`.
All paths use the path convention, with no opaque editor or scope value in Grap.
The Grap-facing jump provides the standard document-path interpretation.
`drawing` accepts width/ascent/descent and a drawing-program value, as the
stored drawing node does; it does not construct a list of draw commands. Its
optional fuel argument configures the later drawing evaluation, as the stored
node's does; omitting it uses the evaluator's ordinary default, not the layout
call's remainder. Every construct is in both forms under one name and the same
fields: a node of the stored form, `{isa: row, gap, children}` say, is a
function of the same name in a layout program, and a wrapper's content is its
`child` field in both. The node's kind and the function are separate cells
sharing that name.
Row/column gap defaults to zero, column baseline to zero, padding sides to zero,
and text paint to the ink face. Other required inputs are validated. Invalid
builder calls return absents; containers propagate a failed child computation
without emitting that container. A final absent, multiple root emissions, or
evaluator halt discards all output. There is no sticky failure flag overriding
Grap recovery. An intentionally empty layout can emit an empty row or column
with `{}` as its child computation; an empty `do` is not a successful computation.

The explicit boundary matters for composition: pane/viewport projection
functions return ordinary values, and no function changes meaning inside a
secretly installed scope. A layout function such as `drawing` or `border` is
one only within a layout program; in ordinary evaluation it is no function at
all. The stored form's nodes are data of their kinds, which the layout library
draws as each kind provides.
Grap can also return stored layout descriptions; a layout program avoids building
and decoding them per node, but still allocates the chosen Rust
layout representation and evaluates Grap code. It is not a cache or a guarantee
of improved speed for arbitrary programs.

## Evaluation

Grap recognizes these forms through its vocabulary cells:

```text
{isa: lambda, params: [parameter-cells...], body: expression}
{isa: call, function: callable-expression, parameter-cell: argument-expression, ...}
{isa: value wrapper, value: expression, ...}
{isa: closure, lambda: {isa: lambda, params: [...], body: expression}, environment: {...}, body origin: ...}
```

A record is a call, a lambda, a value wrapper or a closure only when its `isa`
says so (the shared kind convention, which the `isa` library owns): without
one, a record with a `function` or `params` is data. Each kind is a cell of its
own (`grap::vocabulary::kind`), not the field that shares its name, such as a
closure's `lambda`. So an unfinished one stays what it is, and a field its kind
requires that's missing answers a missing-field absent naming it, where it's
used: a call with no function when it's called, a lambda with no body when
it's applied. A lambda has its parameters, so a call of it its argument slots,
before its body is written.

Evaluating a lambda captures its lexical environment
in a callable value, a closure. Materialized, it is `{lambda, environment, body
origin}`: the lambda as written, the environment as an ordinary GID record of
its bindings, and, when the body has a source, where it came from, so errors
and traces inside a closure called later still point at it. A closure is a
value: as code it matches no form, so evaluating it again gives itself. Grap-defined calls are strict,
evaluating every declared argument before the body. A missing argument binds
its parameter to the missing-argument absent, which answers only where the
body uses it. Effects are explicit
foreign calls and scoped capabilities, as in event handlers. Argument
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

Cell evaluation checks lexical bindings first, then asks `grap::Host::definition`
for one definition: the document's value, otherwise the first loaded library
definition.
Duplicate definitions are tolerated, not merged, composed, or overloaded. Names
and structural display use the same selected definition; duplicate indicators
and inspection UI are [deferred](deferred.md#duplicate-definition-inspection).
A stored `Follow` names the document or a stable library identity, never an
ordinal into the loaded stack, so loading or reordering libraries cannot
silently retarget editor state. With no definition,
evaluation returns missing-cell absent. A cell is transparent: its definition is
evaluated where it is referenced, using the bindings in scope there, so a cell
defined for a function body can be referenced in several places within it. A
foreign function's value is its definition, `{isa: foreign function,
implementation: <cell>, name, params}`, the cell the one the host implements it
under, which is its own: a reference to one evaluates to that data without
invoking anything. Calling one reaches the implementation under that cell, a
scope's first, as a call of the cell does. A scope supplies only
implementations, so a function it lends with no library definition reads as a
missing cell; each library defines its lent functions, with implementations
that answer outside the scope.
The Grap library draws one as a lambda's head: its implementation's name, then
its parameters.

Direct calls retain the same `{function: cell, ...arguments}` syntax for Rust
and Grap implementations. Passing a foreign function through an evaluated
argument passes that record, which still dispatches in the receiving host
context by its cell; it holds no native function pointer. The Fidget examples
name their viewport function this way.

A direct call uses the same resolver. A native definition invokes its Rust
implementation; an ordinary value is evaluated as the callable.
Every result is definitive, including the `declined` absent. A non-callable value
returns a not-callable absent with the offending value; it does not search other
sources for an implementation. An absent callable is its own reason, so a call
of one answers it unchanged, as an absent argument passes on. Scoped capability functions can override this
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
an effect halts evaluation with
`{isa: absent, reason: effectful-decline, value: cause}`.
This contract remains useful to explicit compositions
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
a live evaluation context. They may inspect, return, forward, or evaluate those
raw expressions, in the calling environment or an explicitly derived one. The
environment becomes a GID record only when Rust explicitly hands it to Grap.
A library can instead register one of Grap's primitives (`grap::primitive`),
an operation the evaluator runs natively, with the library's own labels:
every number type's arithmetic, comparisons and one-operand functions are
registered this way. A call that resolves to a primitive runs it directly
rather than through the foreign-call machinery; resolution, shadowing, fuel
and effects are as for any other foreign call.
Semantic failure is an ordinary absent value; the host `Result` only propagates
evaluator halts such as exhausted fuel. A function written in Rust declares
its arguments with `ForeignFunction::with_arguments`: each is found under its
label once per call site, as a primitive's operands are, then evaluated on each
call and parsed into the Rust type the function works with (`Parse`), and the
function's body runs only when every one parses. Otherwise the call returns
"invalid arguments", naming under `call` the origin of the call, where it has
one, and carrying under each label why that argument couldn't be used: a
missing argument, naming its parameter and, under `call`, the origin of the
call it's missing from, where the call has one; its type's reason carrying the
value; or the absent it already was, passed on unchanged as the cause. A call
site's missing arguments are found with the call, once. Each type owns its reason, so a
function taking several types reports each correctly, and a library's own
type, such as a list or a PNG, carries its own reason. So `*` with neither
operand is "invalid arguments" carrying "missing argument" under both `left`
and `right`, and `"three" + 4` carries "not f64", with `"three"`, under
`left`. The failure is reported in the terms of the function that failed,
which needed only its own arguments; it can't say what a caller meant by them.
An absent's causes are the absents among its fields, so they make a tree; a
check for a particular reason looks at its leaves
(`grap::absent::root_causes`). The same absent twice, as in `x * x`, or the
same wrong value on both sides, is carried once, and an absent whose longest
chain of causes is already `CHAIN_LIMIT` long passes on unchanged, so a loop that
keeps using one doesn't grow it a layer per pass. Control functions can
evaluate only the chosen branch. Blobs, lists, and unrecognized records
are inert: the evaluator does not search ordinary containers for expressions.
A function's returned value is not evaluated again.

The registered `evaluate` function is distinct from the projection field. It
accepts an explicit environment value and asks the evaluator to interpret its
raw expression there. It is an ordinary library function.

Fuel limits individual evaluator runs to help catch accidental runaway programs;
it is not a security boundary. Calls within one evaluator context still spend
that context's allowance. Separate projection evaluations start fresh and may
specify their own budgets; the projection context retains no remaining-fuel
state, carries no unused fuel through projected values, rendering, or errors,
and does not clamp a nested render's requested budget. In particular,
an endlessly self-reproducing projection can still hang the editor. Preventing
all such loops is not a contract of the fuel mechanism.

## Control functions and absents

Grap's `all {expressions: [...]}` evaluates independent expressions
in the calling environment, in order, and returns the list of results, including
absents. An empty `all` returns `[]`.
Ordinary absents do not suppress later expressions or roll back earlier effects;
evaluator halts still stop execution. Its compact projection uses the same
expression-list presentation as `do`, with normal expression completions.

The absent library's `or default {value, default}` evaluates `value` once and
returns a non-absent unchanged. Only an absent result evaluates `default`, in the
same calling environment. It does not validate the value's type or undo effects;
actual evaluator halts still halt rather than invoking the fallback.

The absent library draws an absent as its reason's name followed by any other
fields, such as a failed alternative's `causes`, all on a red wash; each cause
is washed in turn inside it. An unnamed reason shows its short cell identity.
An absent's `call` reads as the call itself, drawn where it is: the same place
as the call in its code. The field holds where the call is, which matters only
for what it leads to, so it's drawn as a part of the absent
(`Fields::parts`) and the call is the stop; the callers below are drawn the
same way. Inside a result that can be run again, such as a
pane's render, an absent about a call also has its callers below it, shut: a
"called from" toggle. Opened, they're found by running the result's evaluation
again with an observer (`grap::Answering`) that watches for the call at that
place answering that absent, and each caller is drawn where it is, outermost
last. The run is memoised like the result's own evaluation, and a shut toggle
runs nothing. A result drawn by `crate::display::at_evaluated` carries the
evaluation it came from, so what's drawn inside it can ask. A missing argument reads as its reason and its parameter, drawn as a shallow
reference, then the hole for the argument: a jump to the call's origin and the
parameter's field there, so the error's hole and the call's are one place.
Selecting either marks the other, and filling either fills the call. A call
with no origin, such as one a host makes, has no hole to show.

`do` evaluates its expressions in order, returning the first absent unchanged
or the last successful value. It short-circuits that sequence, not the evaluator:
an enclosing `match` can handle its result. Earlier effects are not rolled back.
An empty `do` returns `missing final expression`. Ordinary `let` and `where`
bindings may bind absents without short-circuiting; a structural binding mismatch
returns a pattern-mismatch absent.

The logic library uses open `{bool: true-cell}` / `{bool: false-cell}` records.
The payload is compared by identity, never evaluated or compared by its name.
Its `require` function takes a `condition`, returning `{}` for true and a
condition-not-met absent for false; a condition that isn't a boolean, or is
absent, makes it "invalid arguments" as for any function. This is an ordinary
guard used in `do`, not evaluator syntax or a configurable bind mechanism.

The list library's `iterate` finishes only when a step returns
an `iteration-finished` absent, and then returns the previous state. An
`unfold` step returns `{item, state}`: its item, if it has one, joins the list,
and a step without a `state` is the last, so `{}` ends it. It returns the
accumulated `list` and the last `state` it was given; a step that isn't a
record is an invalid-step absent. In both, other absents propagate unchanged;
actual evaluator halts still halt. `fold` remains an ordinary fold, allowing
any accumulator value rather than imposing short-circuiting.

The sequence library provides streaming iteration without materializing a list.
A sequence is a zero-argument callable returning a step, `{item, next}`, where
`next` is another zero-argument callable. A step without `next` is the last and
one without `item` yields nothing, so `{}` ends a sequence. Ending isn't a
failure, so it isn't an absent: an absent step is the producer's failure and
propagates unchanged, and a step that isn't a record is an invalid-sequence
absent. The step record is open to unrelated fields.
Calling the same pure closure again returns the same item; consumers advance by
calling the returned successor, not by mutating a cursor. `range {count}` produces
f64 indices from zero up to (excluding) count, a nonnegative integer no greater
than 2⁵³ so every increment remains exactly representable.
`for each {items, action}` pulls one item and calls `action {item}` before pulling
the next; an absent action result stops it, otherwise completion returns `{}`.
`collect {items}` explicitly materializes a GID list. Consumers retain the
current callable and obey ordinary evaluator fuel limits. An empty range is a
callable that immediately returns `{}`.
Grap-authored producers can use the same record/callable protocol; there is no
new evaluator syntax, hidden stream handle, or precomputed list of indices.

Grap's [control module](../progred/src/libraries/grap/control.rs) supplies structural matching
and quote/unquote as ordinary Rust functions using that evaluator interface.
`match` evaluates its subject once, then tries ordered cases, each
`{isa: case, pattern, expression}`. Record patterns are open; list patterns are
exact and ordered; `{isa: binder, bind: cell}` captures a value.
Repeated binders require equal captures, and a final binder is the ordinary
catch-all. Only the selected expression runs,
in the caller's environment extended by captures. Its absent result is final,
not a request to try another case. There is no implicit match-subject binding.

A `match`'s cases and a `let`/`where`'s bindings, each
`{isa: binding, bind | pattern, value}`, are the form's own syntax, read where
they're written and never evaluated, like `do`'s expressions. A clause list
that isn't written in place, such as a reference to a cell holding one,
answers invalid cases or invalid bindings, and a clause that doesn't say its
kind is an invalid case or binding. A record in a pattern is a binder only
when it says so; any other record there is a record pattern.

`quote` walks the original raw structure, replacing `{isa: unquote, unquote: expression}`
leaves by evaluating their expressions in the caller's environment. It never
revisits a spliced result. Outside that traversal, an unquote record is
ordinary data. There is no evaluator-level quote/literal form.

The normal projection shows a compact quote call with a double-quote prefix,
and an `{isa: unquote, unquote: expression}` record with a backtick prefix. The
unquote prefix selects the whole unquote record; its body remains independently
selectable at the stored field path. The expression
uses Grap's local shallow-reference projection, while quoted data keeps its
ordinary projection. Unrelated fields do not block either compact projection;
they follow it as a trailing record, as does a field being added. These markers
are display notation only, not new evaluator or text-bridge syntax.

Grap likewise projects a `match` call with a subject and a
case list as ordered `pattern → expression` cases, and projects
`{isa: binder, bind: cell}` distinctly within patterns. Each arrow targets its expression the way a field
head targets its value. A malformed call declines to the general Grap-call or
structural projection; a malformed case declines to the ordinary element
projection, so every value stays exposed.

Semantic failure is an open GID record of the absent kind:

```text
{isa: absent, reason: reason-cell, ...occurrence-details...}
```

The projection stack draws one as the absent library provides for its kind.
The reason cell supplies names and other static metadata. Occurrence fields
can identify a missing cell, invalid value, or cycle. Code inspects identities,
not human-readable diagnostic text. Hosts and tests inspect the returned value;
there is no parallel Rust diagnostics list that can reject a successful result.
There is no unspecified reason or zero-argument absence constructor. Each
producer chooses a reason, reusing library reasons and adding occurrence details
where useful. Failed atomic-editor conversions include the invalid input.

An unmatched `match` preserves one mismatch unchanged or combines several as
`{isa: absent, reason: no-alternative, causes: [...]}`. This is an ordinary absent, not an
implicit request to try another function definition. A handler using `match`
can explicitly decline with a final catch-all case. A selected expression's
result remains definitive within that match. Absence composition belongs only
to such explicit ordered-choice operations, not to cell lookup; the evaluator
accumulates no ambient failure trace.

Fuel exhaustion halts immediately; at the public boundary it is still an absent
value, with `Evaluation.completed` recording that execution did not finish.
An effectful decline also halts rather than returning normally.
Returning that same value normally still counts as completed execution.
Drawing-source origins are separate from failures.

The evaluator keeps no call stack of its own. A host can offer an observer
(`grap::Observer`), told of each call an evaluation enters and leaves, and
`grap::Observed` attaches one to any host. A drawing's record step attaches a
`grap::Tracer`, which keeps the calls a fill is drawn under and captures the
source-bearing ones as a call trace, sharing prefixes between captures. An
evaluation nobody observes pays one branch per call.

## Equivalent host representations

The runtime can carry unboxed f64, f32 and u64 numbers, booleans and text, and
containers with runtime children. These stay unboxed through calls,
containers, and environments until a generic boundary asks for `Value`, and a
literal is lowered to its unboxed form once. Their encoders and decoders live
with the runtime carrier and are re-exported by the number, logic and text
libraries, which still own their names, projections, and absences. They
accelerate ordinary conventions without adding numeric or string syntax or new
GID primitives. A new privileged convention adds a native form to the runtime's
unboxed values, which answer every record question as their records would.
Privileged knowledge of a library convention is an accelerator only, never a
capability: each such library must remain expressible externally, evaluating
correctly at reduced speed, and reads to Grap as an ordinary library. An
enriched source number or text retains its original value so a pass-through
preserves unrelated fields. Public `evaluate` and `apply` return
`Evaluation<RuntimeValue>`. `RuntimeValue::into_value` (or
`Evaluation::into_value`) explicitly materializes GID; the `*_value` entry
points adapt callers that still need that representation.

Expressions own shared lowered code and source locations. A returned closure
keeps its body and lexical environment alive, not its creating evaluator,
host, compiled thunks, or capabilities. Applying it starts with the receiving
evaluation's execution caches, host, and capabilities. Foreign references
retain a cell identity and resolve its implementation in that receiving
evaluation. Code origins survive; a closure's creation-time call stack is not
part of its invocation-time call stack.

Closure GID records also retain an optional `body origin` field beside
`lambda` and `environment`. It describes the body's source, not the call that created
the closure. The ordinary record forms are:

```text
{root source: path}
{source cell: cell, source path: path}
{input source: path}
```

Paths use the existing path-library convention. The shared codec lives in
`grap::path`; Progred's editor library re-exports it and names its cells. These are ordinary GID records, not new atoms. The editor anchors source
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
adapters serve macros and consumers that read GID.
`ForeignFunction::from_value` is the explicit adapter for a GID-returning
implementation.

Progred carries runtime values through evaluation, projection, and the
libraries. A projection receives a `RuntimeValue`, and descending into a
returned container retains its runtime children. Runtime-aware partials inspect
the value directly; `partial` adapts a GID-oriented projection by handing it the
value's GID view, which borrows stored data as it is and materializes anything
else once, on first request, keeping it for later readers. Values otherwise
become GID only at GID boundaries: copying a value, writing document or UI data,
a library decoding its own data such as a path, paint, tool profile, or radio
options, and worker requests, which contain geometry, never runtime closures.
Callbacks therefore keep their code locations and captures through layout
programs, controls, trees, and previews. The
[runtime-value migration](history/runtime-values-2026-10-19.md) records which
consumers read runtime values directly.

Generated drawing syntax may contain retained native closures. Explicitly
interpreting that syntax lowers its containers without serializing the embedded
closures. The expression-facing host application adapter keeps the usual
contract: Grap parameters bind supplied data, while native functions receive
argument syntax to interpret. This is distinct from `apply`, whose callable and
arguments are already evaluated.

Inline code retained by the editor is anchored to its original document
occurrence before evaluation; cell code retains its cell and path.
Later invocation does not rebase those locations onto the drawing program.
Cmd-hover chooses the innermost call with an available projected occurrence.
Memo input and result comparisons conservatively include native closure
code/capture identity, so closures with different origins never reuse one
another's results. They can decline reuse for separately allocated but
equivalent closures; this is a cheap reuse test, not Grap value equality. A
closure's GID record carries its origin, so GID equality tells such closures
apart too.

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
representations. Unboxing must be transparent to Grap behavior, including
when a value comes through a cell or a constructed container. Origin metadata
preserves the cell and the path relative to its definition. See [source identity](model.md#occurrences-definitions-and-views).

## Examples

[grap-demo.gid](../examples/grap-demo.gid) exercises calls, closures, matching,
quote/unquote, explicit evaluation, and absents. [iop-tree.gid](../examples/iop-tree.gid)
projects an interpreted tree drawing alongside editable source. Headless tests
exercise these paths through the production library set.

Open questions, including address lifetime and hidden active insertions, are in
[deferred work](deferred.md).
