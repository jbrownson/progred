# Layout-derived keyboard navigation

This is the ordinary navigation implementation, replacing the deferred
neighbor-callback system. There is no experimental toggle.

Projections declare selectable stops and their selection callbacks. The layout
interpreter folds those stops into temporary line summaries while placing the
chosen alternative. Selection paths, conjects, and editing capabilities are
unchanged. No coordinates, distances, or measured-height thresholds choose
destinations.

The box engine exposes its chosen structure through `measured::ObserveLayout`:
`layout(composition, children)` and `layout_child(child)` enclose ordinary
placement. Both the choice interpreter and direct measured combinators use
this interface. It adds no callbacks to the retained frame and does not run
widgets twice. Progred consumes that structured traversal; it no longer wraps
each row/column to reconstruct a separate hierarchy. Puri remains layout-neutral.

Columns concatenate their children's content lines and inherit
the logical baseline of their declared baseline child. Baseline-aligned rows
combine children at those baselines; top-aligned rows combine from the top and
retain their declared child's baseline. Empty contributions, including decorative
gaps, do not add navigation lines but retain their position in the layout's child
indexing. Only the chosen alternative places and contributes. Each view
has an independent collection, keyed by occurrence path. Whole-value declarations
enclose their contents; they are not additional children of the layout's columns.

Each open composition folds children as they finish. For each content line it
keeps container-entry levels, with only the first/last stop and, if selected,
the immediate left/right neighbors. Each level also records whether it contains
content or only enclosing whole-value stops. It does not retain the intervening
stops or a navigation tree. Per-line summaries are necessary because painting visits one
whole column before the next, while navigation interleaves their logical lines.
The separate canonical-target table still retains declarations until the view
finishes: repeated declarations at one occurrence refine arrival behavior
without adding another stop. This is not constant-space construction.

A whole-value stop precedes its contents. First, row/column composition aligns
content lines without counting whole-value entry steps. Then, within each
content line, enclosing selections are visited before their contents. A
multiline container introduces an entry level; a single-line container precedes
its contents on the same navigation line. Consecutive enclosing multiline
containers share that entry level: Right still visits each whole-value stop,
but Down enters the first content line directly. Up from that content line
returns to the outermost stop on the shared entry level. An intervening content
stop, such as a function label, ends the chain. None of these entry levels shift
the alignment of neighboring content. A row containing multiline content
remains multiline.

For example, in a row containing `all` beside a vertical expression list, the
label and whole-list stop share a navigation line. The first expression is on
the next navigation line: Right from `all` selects the list, Down reaches the
first expression, and Up goes outside that row. Previously the list's entry
step was inserted before composing the surrounding row, putting the list's
selection above `all`. Explicit offset baselines still align content as declared;
container nesting never contributes to those offsets.

Left/Right visit adjacent stops in logical reading order, wrapping between lines.
Up/Down go to the first stop on the adjacent logical line. Thus Down enters the
first item of a vertical list, but passes over a single-line cell/list to the next
line outside. Consecutive enclosing containers do not add repeated vertical steps.
This deliberately simple policy has no retained column, history, geometric
scoring, or special selection state. Center-aligned rows still combine top-first;
center alignment and overlay/floater navigation have not been given separate
policies. These are the current policies, not restrictions on what the layout
language can express.

After placement the collector scans the line summaries, retaining the previous
line until it reaches the selection and then capturing the following line.
It installs one ordinary `Navigate` handler with the four destinations. The
temporary summaries and stop collection are dropped;
unselected views install no navigation handler. Raw text/picker handlers still
get the first chance at keyboard events, and the existing shell reveals a
navigation destination. Offscreen stops remain available; folded/unplaced
contents do not contribute. Source-less computed results retain their existing
read-only selection behavior.

The anonymous lambda's `λ` marker is clickable to name the function, but does not
contribute an arrow-navigation stop. Stored names remain ordinary text stops;
the whole lambda, parameter list, and body remain navigable. Clicking the marker
still opens the ordinary missing-name picker.

Revealing a destination uses its full painted rectangle, independently of its
logical navigation position. A fully visible selection stays put; a clipped
selection scrolls into view with a small landing margin. When it cannot fit,
the top/left edge takes precedence. The margin is not itself a scroll trigger.

Headless regressions cover multiline cell entry, single-line cells/lists,
nested-list reading order, outline jumps, computed output, text-key precedence,
and independent views. Selection callbacks still encapsulate the destination's
editing context and widget-specific arrival behavior; this change does not
introduce a separate selection representation.

Cmd+5 includes document-authored two-column layouts, both first-baseline-aligned
and last-left/first-right-baseline-aligned, plus three nested cells around a
multiline outline. Headless tests exercise these same stored Grap layout
programs and confirm that their `descend` targets remain editable. The offset
baseline case aligns the right column's first item with the left column's last:
Left crosses that shared line, Up reaches the preceding left item, and Down
reaches the second right item. Tests also cover nested baseline children,
decorative gaps, and entry into multiline containers. Shared collector tests
also cover labels on either side of a container and unequal enclosure depths;
frame tests exercise the ordinary `all` and `do` projections, including cells
around them, without changing their layout. Cells and those calls already use
the same whole-value/row constructs; sharing a vertical entrance is a collector
policy, not a projection-specific navigation rule.

Ordinary lists prefer an inline row and accommodate with a column. Choosing the
inline alternative uses its children's preferred forms; it does not selectively
switch nested ordinary lists to columns to squeeze that row into the available
width. However, a child's preferred form can itself be multiline (an outline,
for example). There is no general single-line-content constraint. A possible
standard-projection rule is to reserve inline sequences for inline children,
while allowing delimiters to enclose a block and explicit layouts to put blocks
side by side. This is not implemented as a new constraint here, nor inferred
from measured height or navigation data.

## Checkpoint measurement

2026-09-27, native ARM64 release build, same machine and unchanged Cmd+9
document. The headless source pane is 600×900 logical points at scale 2,
scrolling a fixed span. Each run has five warm-up frames and 480 measured
frames. Total includes frame disposal, but excludes the 3D preview, GPU
presentation, and platform input delivery. Preserved binaries were run serially,
without concurrent compilation or profiling.

| Run | Implementation | Median frame + disposal | p95 |
| --- | --- | ---: | ---: |
| 1 | Last commit, `d066d51c` | 9.98 ms | 12.05 ms |
| 2 | Layout navigation | 11.57 ms | 13.23 ms |
| 3 | Layout navigation | 11.32 ms | 12.96 ms |
| 4 | Last commit, `d066d51c` | 9.89 ms | 11.56 ms |

This is about 1.5 ms (15%) more CPU time for this workload, not a performance
win. Most of the difference is placement/hover (about 2.1 → 3.3 ms); layout
choices add about 0.25 ms. Preparation and drawing-command emission remain
similar. The new behavior and simpler projection contract are the reason for
this checkpoint, not speed.

The initial dual-engine prototype was slower still. Cleanup removed navigation
wrappers that duplicated ordinary layout, uses one frame-local scratch buffer,
moves owned logical-line vectors rather than rebuilding them, and avoids
re-wrapping baseline-row children: baseline-row composition is associative,
covered by a regression test. There is no cross-frame navigation cache,
custom allocator, or arena.

The separate instrumented source-scroll run measured 208,050 → 228,122
allocations per frame (about 10% more), requesting 30.25 → 32.18 MB (about 6%
more). These measurements are allocation traffic, not retained memory;
instrumented timings are not used in the table above.

Reproduce the timing canary:

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  --config 'env.FRAME_PROFILE_ITERATIONS="480"' \
  cam_source_scroll_profile_loop -- --ignored --nocapture --test-threads=1
```

Use `--features layout-profile` and `cam_source_scroll_form_profile` for the
separate allocation breakdown. The new `navigation_allocation_profile` canary
also measures a root-selected frame. It is not the workload in the table above.

Validation: 938 Progred tests pass (55 diagnostic tests ignored), including
raw-key precedence, arrival direction, alternative selection, nested outlines,
jump editing, computed results, empty containers, baseline alignment, and
independent panes. All 70 Puri tests pass (one diagnostic test ignored), and the
threaded WASM library check passes. No completion policy changed in this
checkpoint; the anonymous-marker stop was removed in a subsequent change.

## Initial container-wrapper prototype

The wrapper-preserving collector passes all 951 Progred tests (56 diagnostic
tests ignored), including the new label/list regression. The threaded browser
build also succeeds. No projection API or `all`/`do` layout changed.

The same 480-frame source-scroll canary was run with a preserved pre-change
binary and the prototype, serially without concurrent builds/tests. Final paired
before/after medians were 12.30/12.17 ms and 12.25/11.33 ms. Earlier samples went
the other way (11.96 ms before versus 12.35–12.63 ms after). The variation does
not establish a small speedup or slowdown. This is the unselected source-scroll
workload: it includes collecting/discarding navigation fragments, but does not
measure resolving destinations for an active selection.

## Chosen-structure streaming follow-up

The production collector now uses the box engine's `ObserveLayout` traversal
and incrementally folds line summaries. The prior full-tree implementation is
retained only as a test oracle. Generated mixed layouts compare the streaming
composition and all four destinations against that reference, in addition to
the existing editor regressions. All 953 Progred tests and 29 measured tests
pass (56 Progred diagnostic tests ignored). Tests cover discarded alternatives,
omitted/deferred placement, independent views, and repeated stop declarations.

2026-09-28, the same unselected source-scroll workload, with preserved release
binaries and no concurrent builds/tests: before/after/after/before median
frame-plus-disposal times were 11.68/10.85/11.83/11.23 ms. Corresponding p95s
were 13.86/13.10/12.92/13.91 ms. This is roughly flat within run-to-run variation,
not evidence of a speedup. No selected-frame or allocation-count comparison
has been made for this follow-up. The motivation is the explicit structural
boundary and removal of the reconstructed navigation tree.
