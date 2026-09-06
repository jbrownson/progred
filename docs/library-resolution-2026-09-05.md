# Unified library definitions — 2026-09-05

Measured on the local ARM64 Mac using release builds under Seatbelt; no app
launch. The baseline was the working tree after the first-definition lookup
simplification, before unifying the stored definitions and moving library
descriptions into ordinary cells. These timings do not compare against the
older multi-definition dispatch implementation.

## Change

Libraries now store one sorted, copy-on-write definition table. A definition
contains either an ordinary Value or a shared native payload containing both
its descriptive Value and its Rust implementation. `Host::resolve` is the
single runtime query used by cell reads and calls. Built-in data and function
declarations are joined once when the library is constructed.

On this target, both `Value` and `Definition` occupy 24 bytes. Native entries
have one additional shared allocation for their combined payload; ordinary
entries do not grow. The document's GID cell table is unchanged.

Library descriptions are ordinary cells under their library identities.
Repeating a library identity replaces all contributions in place before
projections and completion providers are composed. No cross-frame cache,
implicit library loading, or evaluator loading capability was added.

## Measurements

Whole-loop averages, including the first frame and output destruction:

| Run | Picture, 100 frames | Source view, 30 frames |
| --- | ---: | ---: |
| Before, first | 23.9 ms | 5.7 ms |
| Before, repeat | 24.7 ms | 6.0 ms |
| After, first | 24.3 ms | 5.6 ms |
| After, repeat | 25.8 ms | 6.4 ms |
| After, third | 25.9 ms | 6.7 ms |
| After, no rebuild | 25.4 ms | 7.9 ms |

The picture workload remains in the same approximate range. The later source
runs were slower than the earlier ones; the last includes several 14–17 ms
frames amid mostly 5–6 ms frames. These sequential runs are not a controlled,
interleaved A/B test and do not establish whether a modest source-view
regression exists. The first comparison was effectively unchanged; do not
interpret that alone as proof of zero overhead, or attribute the later spread
to the representation change without a controlled follow-up.

The picture benchmark evaluates and records the complete drawing each frame,
then replays into a headless DrawList. The source benchmark clips the document
to 1400 × 900. Both retain app-lifetime font and library resources; approved
text shaping warms normally. Neither measures GPU rendering, presentation,
input scheduling, or an entire interactive window.

## Validation

All 464 workspace tests passed (two profiling tests ignored). New tests cover
copy-on-write definitions, native descriptions, ordinary library references,
and replacement of the complete contribution, including every completion
category and projections. The web target compiles with the existing
`drawn_menu` and `Quit` unused-code warnings.

Reproduce:

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  --config 'env.IOP_PROFILE_ITERATIONS="100"' \
  iop_tree_ -- --ignored --nocapture --test-threads=1
```

## Contextual completion follow-up

After replacing the separate completion categories with one contextual provider
request, all 470 workspace tests pass (the same two profiling tests ignored).
Providers remain inactive outside the focused picker. One headless source run
averaged 6.5 ms over 30 frames, including a 28.1 ms initial frame; subsequent
frames took 5.1–6.6 ms. This is another sanity check, not an interleaved A/B test.

## One partial projection per library

A subsequent change replaces each library's public list of partial projections
with one `Partial`. Libraries compose multiple forms using
`compose_partials`; first success wins, and an empty composition declines.
The editor still composes library contributions in load order over the total
structural fallback. Composition retains its callback list once, not on each
projection; singleton compositions return the supplied callback directly.
No computation cache or relevance shortcut was added.

This comparison saves the pre-change release test executable and alternates it
with the changed executable under the same Seatbelt wrapper. The baseline
already includes the contextual completion changes described above. Each run
uses 100 picture frames and 30 source frames at the same 1400-pixel width.
Rows 1–4 ran before/after; rows 5–6 reversed that order. Numbers below are
whole-loop mean milliseconds, including the initial frame and destruction.

| Pair | Source before | Source after | Picture before | Picture after |
| --- | ---: | ---: | ---: | ---: |
| 1 | 7.0 | 7.2 | 26.8 | 34.4 |
| 2 | 6.7 | 6.7 | 36.7 | 28.2 |
| 3 | 8.3 | 10.0 | 26.2 | 31.8 |
| 4 | 6.5 | 6.4 | 31.4 | 31.4 |
| 5 | 7.8 | 6.1 | 26.0 | 26.6 |
| 6 | 6.9 | 6.6 | 26.0 | 25.6 |

The median of the six source loop means is 6.95 ms before and 6.65 ms after;
there is no detected source-view regression. Picture results are less clear:
the corresponding medians are 26.5 ms and 29.8 ms, with outlier frames above
100 ms in both executables. The last two reverse-order pairs converge around
26 ms, and typical per-frame timings also vary between runs. These results
do not establish a repeatable picture slowdown, but do not justify claiming
zero overhead either. No timing improvement is attributed to this refactor.

All 473 workspace tests pass, including ordered, short-circuiting composition,
identity, regrouping, and all-declining fallback checks. The web target still
compiles with the same two existing unused-code warnings.
