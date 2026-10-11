# Native CAM source scrolling — 2026-09-26

## Finding

Repeated release measurements show a modest source-frame regression since the
pre-website checkpoint: roughly 1.5–2 ms, or about 20%. They do **not** isolate
the web changes as its cause. This interval also includes runtime-value,
provenance, projection, and navigation work. The latest neighbor-callback
navigation substantially improves on the preceding full-graph implementation.

This investigates scrolling the source document, not wheel zoom in the preview.
The user's exact preview mode and viewport size were not confirmed. These are
headless CPU measurements, not a recording of the reported live interaction.

## Comparison

Same `examples/toolpaths.gid` in all three revisions; native ARM64 release
builds on the same machine, macOS 27.0. Source-only viewport: 600×900 logical
points at scale 2. Five warm-up frames, then 480 measured frames per run.
Scroll follows a fixed 1200-point span and returns, without changing viewport
size. All revisions received the same scrolling addition to the existing
source-frame canary.

Preserved binaries were run sequentially in the order below, under the
repository's Seatbelt policy, to distinguish the revision difference from
run-to-run noise. These runs were not sampled by the profiler.

| Run | Revision | Median | p95 |
| --- | --- | ---: | ---: |
| 1 | `127792e7`, before website work | 9.06 ms | 10.01 ms |
| 2 | `0e71e9e5`, current | 10.53 ms | 11.84 ms |
| 3 | current | 11.29 ms | 12.21 ms |
| 4 | before website work | 9.36 ms | 10.13 ms |
| 5 | `bafda915`, preceding navigation graph | 17.52 ms | 19.36 ms |
| 6 | current | 10.59 ms | 11.97 ms |

Initial, unpaired runs looked nearly equal (about 9 ms); the alternating runs
above supersede that preliminary impression. The current code is about 40%
faster than the preceding navigation-graph checkpoint in this benchmark.

Representative phase medians (runs 1, 5, and 6):

| Phase | Before website | Previous nav graph | Current |
| --- | ---: | ---: | ---: |
| Projection preparation | 5.69 ms | 6.22 ms | 6.36 ms |
| Layout choices and settled geometry | 0.79 ms | 0.98 ms | 1.08 ms |
| Placement and hover | 1.60 ms | 3.06 ms | 2.33 ms |
| After-hover binding | 0.27 ms | 6.73 ms | 0.28 ms |
| Paint and handler disposal | 0.24 ms | 0.27 ms | 0.17 ms |
| Output disposal | 0.12 ms | 0.20 ms | 0.21 ms |

Medians of individual phases need not sum to the median total. The former
navigation graph's resolution is included in after-hover binding. The remaining
pre-website-to-current increase is concentrated in preparation, layout, and
placement, not drawing-command emission.

## Full editor and sampling

The current full-editor canary, with the default Model preview present at
1500×900 logical points and scale 2, measured 13.05 ms median, 14.54 ms p95,
and 18.72 ms maximum over 120 measured frames after five warm-ups. The first
frame was 251.68 ms. This records draw commands but does not submit GPU work
or present a window, and directly changes scroll rather than dispatching
physical input. It is not a historical full-editor comparison.

The warm CPU cost alone approaches a 16.7 ms (60 Hz) frame budget and exceeds
8.3 ms (120 Hz). GPU presentation and scheduling still need to be measured
before claiming an explanation of all the perceived lag. Do not subtract the
source-only timings from this result to estimate preview cost: the viewports
and resulting source layouts differ.

A separate five-second macOS `sample` of the current source canary found most
active samples under projection preparation, with allocation/free, hashing,
copying, record lookup, source/path lookup, and layout work prominent. This
source-only test intentionally excludes Fidget. It identifies useful areas
for follow-up, not an individual offending allocation or proof of duplicated
work. Timing from the sampled run is excluded from the comparison above.

## Reproduction and next steps

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  --config 'env.FRAME_PROFILE_ITERATIONS="480"' \
  cam_source_scroll_profile_loop -- --ignored --nocapture --test-threads=1

./tools/sandbox-cargo test --release -p progred --lib \
  --config 'env.FRAME_PROFILE_ITERATIONS="120"' \
  cam_editor_scroll_profile_loop -- --ignored --nocapture --test-threads=1
```

Historical builds were made from archived revisions with the same canary
addition. When sharing a Cargo target directory between archives, refresh
local source timestamps before rebuilding: otherwise cached newer local
dependencies can incorrectly survive a switch to an older archive. Retain
each resulting binary before building another revision.

Next useful work is to isolate the extra preparation/placement cost, and to
sample an actual native scroll interaction if CPU reductions do not explain
the experience. No production optimization, frame reuse, or invalidation
exception was added in this investigation. The first round added two ignored
canaries and this report; the follow-up below adds opt-in diagnostic scopes.

## Follow-up: isolate runtime ownership from navigation

The remembered evaluator slowdown is a different workload. The same source
scrolling canary was added to three more archived revisions, with unchanged
toolpath input. Initial 480-frame runs measured:

| Revision | Median | p95 |
| --- | ---: | ---: |
| `15985033`, immediately before owned runtime results | 8.76 ms | 9.34 ms |
| `9c676235`, immediately after owned runtime results | 8.89 ms | 9.38 ms |
| `f927dc3f`, all runtime migrations, before navigation work | 9.02 ms | 9.40 ms |

These are sequential post-build observations. A subsequent serial run of the
preserved binaries, with no concurrent compilation or sampling, measured:

| Run | Revision | Median | p95 |
| --- | --- | ---: | ---: |
| 1 | Before ownership | 8.17 ms | 8.99 ms |
| 2 | After ownership | 8.22 ms | 9.03 ms |
| 3 | After runtime plumbing, before navigation | 8.35 ms | 9.30 ms |
| 4 | Current | 9.38 ms | 10.48 ms |
| 5 | After runtime plumbing, before navigation | 8.31 ms | 9.40 ms |
| 6 | Current | 9.62 ms | 10.66 ms |

Absolute timings vary between batches, but the within-batch difference is
consistent: about 1.0–1.3 ms (12–16%) after the navigation work. The earlier
pre-website comparison found roughly 20%. Neither comparison supports treating
this as the accepted evaluator-ownership tradeoff. Source scrolling is still
near its old baseline after all the runtime migrations.

This narrows the remaining regression to the navigation implementation and
the accompanying editing-scope encapsulation; it is not a one-function causal
proof. The scope refactor preserves the previous prefix-test/path-construction
algorithm. Navigation adds per-projection wrappers, layout-choice nodes,
selection callbacks, and per-child neighbor-provider construction. In the
last pair, preparation increased from 5.48 to 6.00 ms, choices from 0.66 to
0.86 ms, and placement from 1.52 to 2.10 ms. Paint emission became cheaper,
not more expensive.

The allocation canary adds the same scrolling input to the existing
`layout-profile` allocator/scopes. Over 120 warm frames it counted about
211,786 allocation/reallocation requests and 31,124,261 requested bytes per
frame. These are cumulative allocation requests, **not** live memory, allocator
footprint, or a leak. Projection preparation and recursive program adapters
accounted for about 55% of requests; placement about 13%. The benchmark has no
selection or pointer and still prepares the whole expanded source layout.

Test-only scopes additionally distinguish `Sources::resolve_path`,
`Scope::source` (conject resolution), and navigation's `begin_child`/`end_child`.
The last scope does not cover all navigation cost: projection wrappers and
layout-choice nodes are built earlier, and some group routing is outside it.
All diagnostic instrumentation is gated by `cfg(all(test, feature =
"layout-profile"))`; normal app builds have no added counters or timers.

The more detailed 120-frame instrumented run reported:

| Scope | Calls/frame | Allocation requests/frame | Requested bytes/frame | Instrumented time/frame |
| --- | ---: | ---: | ---: | ---: |
| Document path lookup | 6,911 | 0 | 0 | 0.573 ms |
| Conject resolution | 9,764 | 9,110 | 1,981,536 | 0.486 ms |
| Navigation child begin/end | 4,282 | 5,312 | 379,704 | 0.227 ms |

Scope timing includes diagnostic overhead and must not be directly subtracted
from the uninstrumented benchmark. Navigation requests originate only at the
selected target, but `begin_child` unconditionally allocates its pending
receiver holder and constructs applicable directional providers. This happens
even in this no-selection workload. Its measured 0.227 ms is only that narrow
part, not the whole extra millisecond.

The next experiment should reduce eager navigation scaffolding while preserving
the callback/combinator interface, then compare both selected and unselected
frames. Path reconstruction is a separate measurable opportunity: ground
classification, writability, child resolution, and secondary-source attribution
ask for related location information. Avoid repeating those resolutions where
one projection can reuse the answer; custom conjects and jumps must retain
their current semantics. These are hypotheses for follow-up, not implemented
optimizations or justification for adding cross-frame caches.

All 14 frame-level neighbor-navigation tests passed with profiling enabled.

```sh
./tools/sandbox-cargo test --release -p progred --lib --features layout-profile \
  --config 'env.FRAME_PROFILE_ITERATIONS="120"' \
  cam_source_scroll_form_profile -- --ignored --nocapture --test-threads=1
```

## Small follow-up: reuse one resolved source locally

`prepare_value` now resolves its occurrence's document source once and lends
that answer to ground classification, writability, and secondary-source
attribution. Previously those independently called the same conject. This is
a local variable during preparation, not a cross-frame memo or a replacement
for resolving edits at dispatch. Custom conjects, jumps, computed values, and
the identity-scope attribution fast path retain their existing behavior.

Alternating preserved uninstrumented release binaries, using the same
480-frame source-scrolling canary with no concurrent compilation:

| Run | Version | Median | p95 |
| --- | --- | ---: | ---: |
| 1 | Before local reuse | 9.39 ms | 10.66 ms |
| 2 | With local reuse | 9.29 ms | 10.31 ms |
| 3 | Before local reuse | 9.75 ms | 10.97 ms |
| 4 | With local reuse | 9.32 ms | 10.32 ms |

This is a modest 0.1–0.4 ms (roughly 1–4%) improvement in these runs, not a
recovery of the whole navigation-period regression or a measured live-frame
speedup. Preparation medians fell from 5.89/6.06 to 5.67/5.69 ms.

The separate 120-frame allocation canary confirms the removed work:
conject calls fall from 9,764 to 5,179 per frame, with 4,299 fewer allocation
requests and 930,552 fewer requested bytes per frame. Other scope allocation
counts are unchanged. Total requests fall from about 211,786 to 207,487;
this is allocation traffic, not a claim about retained memory.

Kept this small change. Deferred navigation-provider allocation changes:
they require more restructuring than this local reuse, and the narrow
begin/end scope is only about 0.23 ms. No arenas, navigation redesign,
cross-frame caching, or scroll-specific invalidation were added.

Validation: all 914 active Progred library tests passed (55 ignored), including
custom conjects, jumps, computed read-only results, source highlighting, and
neighbor navigation. The changed production file passes its formatting check;
the repository-wide check reports pre-existing formatting differences in
unmodified files, which were left alone.
