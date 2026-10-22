# Layout-derived keyboard navigation

Every drawn occurrence is a selectable stop, named by its path. While the
chosen layout alternative is placed, the collector folds those stops into
temporary line summaries. No coordinates, distances, or measured-height
thresholds choose arrow destinations; Tab's order (below) comes from painted
rectangles. Selection paths and editing capabilities are unchanged.

## Structure

The box engine exposes its chosen structure through `measured::ObserveLayout`:
`layout(composition, children)` and `layout_child(child)` enclose ordinary
placement. Both the choice interpreter and direct measured combinators use this
interface. It adds no callbacks to the retained frame and does not run widgets
twice. Puri remains layout-neutral.

Columns concatenate their children's content lines and inherit the logical
baseline of their declared baseline child. Baseline-aligned rows combine
children at those baselines; top-aligned rows combine from the top and retain
their declared child's baseline. Empty contributions, including decorative gaps,
add no lines but keep their position in the layout's child indexing. Only the
chosen alternative places and contributes. Each view has an independent
collection.

## Whole values and blocks

A whole-value stop precedes its contents. If the contents fit on one line, the
stop joins that line. If they span several lines, they form a block: the stop
gets an entry line above the block's first drawn line. Entry lines never shift
the alignment of neighboring content.

Within one drawn line, stops before the first block join its entry line, and
stops after it stay on the line they are drawn on. For `all` beside a vertical
expression list, `all` and the list share the entry line, and Down reaches the
first expression. For a vertical list with a label to its right, Right goes
list → first item → label → second item. Blocks side by side share their entry
line. Directly enclosed blocks share one entry line too: Right visits each
whole-value stop, Down enters the first drawn line, and Up from there returns
to the outermost stop. A stop before the block, such as a function label, ends
that sharing. An enclosing stop always begins the topmost entry line, so it
precedes a deeper block that sits beside its first one.

## Stops

Every occurrence drawn is an arrow stop, unless its projection draws it as a
part of the occurrence around it (below). An occurrence's landmark adds a
whole-value stop before what's drawn
inside it, unless a control drawn for the occurrence, a line edit or a picker,
declares the occurrence's stop itself. That stop stays where the control is
drawn, as a number's does after its name, and the landmark adds no second one;
a landmark nested directly in another at the same occurrence, as a picker's is,
shares its stop. So cells, lists, records, custom forms, and values drawn by a
Grap projection function all stop the same way. What a projection draws as part
of its own occurrence, such as a call's argument record or an outline's extra
fields, adds no stop, and punctuation, padding, and other inert decoration add
no stops or lines. Empty lists stop once; empty cells offer both the reference
and the missing contents.

Each stop draws something of its own, so selecting it looks different from
selecting what's inside it, and every arrow press shows. Where what's inside
would draw all of an occurrence, its projection marks it with a handle from the
toolkit: a bar as tall as what it draws, before it, that a click on selects, as
a notebook has. It always shows: structure seen only once selected isn't
visible. Other marks serve too: a notebook numbers its entries in its
list's margin, and an entry shows its arrow before it has a result. A sweep
selects each stop and checks that its box is neither the box of the stop around
it nor of one just inside it, over the sample and a new notebook, and, ignored
for its time, the larger examples.

A domain projection presents the domain's things, and a level someone working
in it wouldn't act on alone isn't a stop: it's drawn as a part of the thing
around it (`part`, a walk like `descend`). A part has no stop, highlight, or
landmark of its own, so arrows pass from the thing to what's inside the part,
which keeps its own stops; a control drawn for the part itself, such as the
hole of a definition yet to be written, still declares one there. A Grap
parameter is a cell and its definition record as one thing, its name a stop
inside; a notebook is its record and its list of entries, each entry a stop
inside. The underlying data stays a view away, in Raw. Backspace landing on its
container skips a part for the thing around it. The aim is that the domain
views make an invalid state hard to reach and easy to leave: a parameter's
definition can't be selected or deleted alone, and one with none is a hole
whose offer, the name typed, defines it. What a shape requires, such as a
call's function, is drawn `required`: deleting it leaves a hole, whose
Backspace steps back rather than removing anything, so a call is never left
an empty record.

A selection on nothing drawn, such as one inside a value its projection draws
whole, as a number draws its spelling rather than its encoding, leaves arrows
as no selection does: the first selects the root.

## Arrow policy

Left/Right visit adjacent stops in reading order, wrapping between lines. Up/Down
go to the first stop on the adjacent line. Thus Down enters the first item of a
vertical list, but passes over a single-line cell/list to the next line outside.
There is no retained column, history, geometric scoring, or special selection
state. Center-aligned rows combine top-first; center alignment and
overlay/floater navigation have no separate policies. Floating content (menus,
completions) has its own collection and its own keyboard handling.

Modified arrows that a text field declines navigate like plain arrows, so
holding Option to move by words through text continues into structure.

## Collection

Each open composition folds children as they finish. A summary line keeps only
its first and last stops and, if it holds the selection, the selection's
immediate neighbors; intervening stops are dropped once passed. Per-line
summaries are needed because painting visits one whole column before the next,
while navigation interleaves their lines. Entry lines count up from each row's
drawn line, and leading stops remember the first block's entry line, so
regrouping a row's children never changes navigation.

After placement the collector scans the summaries and installs one ordinary
`Navigate` handler with four destination paths. Unselected views install none.
An arrow arrives through the destination's landmark in that view, the same
landmark that reveals it: the landmark owns selecting the occurrence, including
a line field placing its caret at the edge the arrow came from. A stop without a
landmark declines rather than selecting something that cannot be revealed.
Offscreen stops remain available; folded or unplaced contents do not contribute.
Stops reached through `jump` keep their editing context; computed results,
including those reached through `at`, keep their read-only selection behavior,
apart from a cell's definition followed from one.

Every handler in the frame sees an arrow first, modifiers included, so text and
picker handlers can claim it. Only an arrow nothing claims reaches the editor's
structural keys, the frame's lowest-priority handler. That handler lowers it to
`Event::Navigate(direction)`, dispatches it through the whole frame handler, and
reveals the selection it lands on. With no selection, a root navigation handler
selects the document view's outermost drawn occurrence. Landmarks also serve
pointer and source selection and the explicit Select All command.

Every occurrence the engine draws gets a landmark: the view's root, and each
value a projection draws through `descend` or its peers (`at`, `jump`). What a
projection draws itself, such as a number's digits, gets none. A view showing
a workspace record starts at its main value, so the record isn't drawn and
nothing selects it.

Tab moves to the next tab stop, a place to fill in, and Shift+Tab to the one
before, in reading order within the selection's view: line by line, where a
line is what overlaps vertically, then left to right. A widget marks its own
occurrence as a tab stop, as it registers an arrow stop. The picker marks
every empty, writable hole it draws, optional ones such as a value's missing
name included, so navigation needn't know which fields are optional.
With nothing selected, Tab starts at the first. In a picker, Tab first commits
what was typed, as Enter would. It stays there when that opened a slot of its
own, such as a new call's first argument, or when no tab stop is left, so the
commit shows before focus moves on. A Tab with nothing to commit and nowhere
to go declines, so a browser moves focus on to the rest of its page, which
clears the selection. A window has nowhere else to go, so there that Tab clears
the selection itself, and the next starts over from the first.

The anonymous lambda's `λ` marker is the missing name's place, an arrow stop
like any drawn occurrence. Arriving there types into the name as a line, with
no completion; it isn't a tab stop, since the name is optional.

Revealing a destination follows [selection and reveal](puri.md#selection-and-reveal).

## Layout interaction

Ordinary lists prefer an inline row and accommodate with a column. Choosing the
inline alternative uses its children's preferred forms, which can themselves be
multiline (an outline, for example). There is no general single-line-content
constraint.

## Examples and tests

[examples/navigation.gid](../examples/navigation.gid) includes nested cells around a
multiline outline, document-authored side-by-side and offset-baseline columns,
and content after a block; [examples/README.md](../examples/README.md) says what
to try. Collector tests in `navigation/logical/tests.rs` drive the collector
through shapes mirroring the layout traversal; frame tests exercise the same
policies through real projections and the navigation example document. A
sweep selects every occurrence the sample draws and checks some arrow leaves
it; an ignored one sweeps the larger examples.

Navigation's frame cost shows in `make perf`'s `cam_source_scroll_profile_loop`
canary; see [performance checks](performance.md). Earlier measurements and
prototype notes are in [history](history/navigation-layout-2026-09.md).
