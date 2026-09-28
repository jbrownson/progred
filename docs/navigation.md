# Layout-derived keyboard navigation

This is the ordinary navigation implementation, replacing the deferred
neighbor-callback system. There is no experimental toggle.

Projections declare selectable stops and their selection callbacks. The layout
interpreter combines those stops into temporary logical lines while placing the
chosen alternative. Selection paths, conjects, and editing capabilities are
unchanged. No coordinates, distances, or measured-height thresholds choose
destinations.

Small hooks in Progred's row/column layout interpreter compose temporary logical
lines during placement. Columns concatenate their children's lines and inherit
the logical baseline of their declared baseline child. Baseline-aligned rows
combine children at those baselines; top-aligned rows combine from the top and
retain their declared child's baseline. Empty contributions, including decorative
gaps, do not add navigation lines but retain their position in the layout's child
indexing. Only the chosen alternative places and contributes. Each view
has an independent collection, keyed by occurrence path.

A whole-value stop precedes its contents. When those contents have more than one
logical content line, the whole-value stop occupies an additional leading
navigation line. For single-line contents it precedes them on that same line.
The extra navigation line does not count when determining whether contents are
multiline, and shifts the stored baseline index so the contents still align with
neighboring content. A row containing multiline content remains multiline.

Left/Right visit adjacent stops in logical reading order, wrapping between lines.
Up/Down go to the first stop on the adjacent logical line. Thus Down enters the
first item of a vertical list, but passes over a single-line cell/list to the next
line outside. Nested multiline containers can each contribute an entry step.
This deliberately simple policy has no retained column, history, geometric
scoring, or special selection state. Center-aligned rows still combine top-first;
center alignment and overlay/floater navigation have not been given separate
policies. These are the current policies, not restrictions on what the layout
language can express.

After placement the collector selects four destinations for the current
selection and installs one ordinary `Navigate` handler. The temporary stop collection is dropped;
unselected views install no navigation handler. Raw text/picker handlers still
get the first chance at keyboard events, and the existing shell reveals a
navigation destination. Offscreen stops remain available; folded/unplaced
contents do not contribute. Source-less computed results retain their existing
read-only selection behavior.

Revealing a destination uses its full painted rectangle, independently of its
logical navigation position. A fully visible selection stays put; a clipped
selection scrolls into view with a small landing margin. When it cannot fit,
the top/left edge takes precedence. The margin is not itself a scroll trigger.

Headless regressions cover multiline cell entry, single-line cells/lists,
nested-list reading order, outline jumps, computed output, text-key precedence,
and independent views. Selection callbacks still encapsulate the destination's
editing context and widget-specific arrival behavior; this change does not
introduce a separate selection representation.

Cmd+0 includes document-authored two-column layouts, both first-baseline-aligned
and last-left/first-right-baseline-aligned, plus three nested cells around a
multiline outline. Headless tests exercise these same stored Grap layout
programs and confirm that their `descend` targets remain editable. The offset
baseline case aligns the right column's first item with the left column's last:
Left crosses that shared line, Up reaches the preceding left item, and Down
reaches the second right item. Tests also cover nested baseline children,
decorative gaps, and the synthetic entry lines of multiline containers.

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
threaded WASM library check passes. The anonymous-lambda picker issue remains
deferred; no completion policy changed in this checkpoint.
