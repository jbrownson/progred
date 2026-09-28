# Layout-derived keyboard navigation

Projections declare selectable stops and their selection callbacks. While the
chosen layout alternative is placed, the collector folds those stops into
temporary line summaries. No coordinates, distances, or measured-height
thresholds choose destinations. Selection paths, conjects, and editing
capabilities are unchanged.

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
that sharing.

Repeated declarations of the innermost open whole value's occurrence refine its
arrival behavior instead of adding a second stop.

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
`Navigate` handler with four destinations. Unselected views install none. Raw
text/picker handlers get the first chance at keyboard events, and the shell
reveals the destination. Offscreen stops remain available; folded or unplaced
contents do not contribute. Source-less computed results keep their read-only
selection behavior.

The anonymous lambda's `λ` marker is clickable to name the function but is not
an arrow stop; the whole lambda, parameter list, and body remain navigable.

Revealing a destination uses its full painted rectangle, independently of its
navigation position. A fully visible selection stays put; a clipped selection
scrolls into view with a small landing margin. When it cannot fit, the top/left
edge takes precedence. The margin is not itself a scroll trigger.

## Layout interaction

Ordinary lists prefer an inline row and accommodate with a column. Choosing the
inline alternative uses its children's preferred forms, which can themselves be
multiline (an outline, for example). There is no general single-line-content
constraint. A possible standard-projection rule is to reserve inline sequences
for inline children, while allowing delimiters to enclose a block and explicit
layouts to put blocks side by side. This is not implemented.

## Examples and tests

**Examples → Keyboard Navigation** (Cmd+5) includes nested cells around a
multiline outline, document-authored side-by-side and offset-baseline columns,
and content after a block; [examples/README.md](../examples/README.md) says what
to try. Collector tests in `navigation/logical/tests.rs` drive the collector
through shapes mirroring the layout traversal; frame tests exercise the same
policies through real projections and the Cmd+5 document.

The timing canary for navigation's frame cost:

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  --config 'env.FRAME_PROFILE_ITERATIONS="480"' \
  cam_source_scroll_profile_loop -- --ignored --nocapture --test-threads=1
```

Earlier measurements and prototype notes are in
[history](history/navigation-layout-2026-09.md).
