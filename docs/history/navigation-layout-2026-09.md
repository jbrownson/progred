# Layout navigation measurements, 2026-09

Measurement and prototype notes moved from `docs/navigation.md`. They describe
earlier revisions of the collector; see that document for current behavior.

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
