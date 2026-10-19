# Layout-derived keyboard navigation

Projections declare selectable stops by occurrence path. While the
chosen layout alternative is placed, the collector folds those stops into
temporary line summaries. No coordinates, distances, or measured-height
thresholds choose arrow destinations; Tab's order (below) comes from painted
rectangles. Selection paths, conjects, and editing capabilities are unchanged.

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

A repeated declaration of the innermost open whole value's occurrence is the
same stop, not a second one.

## Projection stops

`display::group(content)` adds the whole-value stop before its
contents; cells, lists, records, and custom forms all use it. `group_hug`
combines a selectable whole with inline or indented head-body alternatives;
ordinary `row` and `col` need no navigation-specific counterpart. A call groups
its function and argument fields together, and its arguments use
`record_fragment_with` to arrange fields without another whole-call stop. An
outline groups its heading and body in a column, then composes sections and
extras vertically; extras use `record_fragment` so they add no second
root-record stop. Empty lists stop once; empty cells offer both the reference
and the missing contents. A leaf facet can supply its widget-specific selection
callback at the occurrence of an enclosing whole-value stop without adding a
stop. Punctuation, padding, and other inert decoration declare no stops or lines.

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
selects the document's root occurrence. Landmarks also serve pointer and source
selection and the explicit Select All command.

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

The anonymous lambda's `λ` marker is clickable to name the function but is
neither an arrow stop nor a tab stop; the whole lambda, parameter list, and
body remain navigable.

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
policies through real projections and the navigation example document.

Navigation's frame cost shows in `make perf`'s `cam_source_scroll_profile_loop`
canary; see [performance checks](performance.md). Earlier measurements and
prototype notes are in [history](history/navigation-layout-2026-09.md).
