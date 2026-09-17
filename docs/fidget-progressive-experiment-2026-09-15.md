# Retained progressive Fidget experiment — 2026-09-15

This is an experiment, **not the application's renderer**. The renderers
are confined to the test-only
[raster diagnostics](../progred/src/libraries/fidget/raster/diagnostics/progressive.rs).
The retained-renderer experiments did not change production scheduling, quality
defaults, dependency versions, or UI. A subsequent app scheduling trial is
recorded at the end of this report.
The persistent-expression follow-up adds three small Fidget core APIs, described
below; the app does not use those APIs.

## Question and method

Can we publish coarse samples, retain the spatial work that made them cheap,
and continue toward a fine result instead of rerendering independent grids?

The prototype fixes the camera, scene, and **final** voxel grid, including the
production 4× depth resolution. It samples that grid at strides 16, 8, 4, and 1.
These are voxel strides, not image dimensions. Certified empty/full regions
and simplified programs survive between stages. Unknown regions are classified
when reached. The final stage uses the same sample positions, interval tile
sizes, depth rules, normals, and object precedence as production.

On Apple Silicon the interval hierarchy remains Fidget's `[64,16,8]`. This is
not an octree subdivided all the way to individual samples: within an 8³ leaf,
bulk sampling preserves Fidget's efficient evaluator interface. Coarse samples
are expanded into blocks for display. They are **not** geometric proofs, and
each stage rebuilds its depth buffer so a coarse miss or occluder cannot hide
a feature at finer resolution. Exact point samples are reevaluated; this first
experiment reuses interval classifications and spatial programs, not all prior
numerical results.

The simple retained representation is a tree per object/image tile, with
unknown, empty, full, and boundary cases. Boundary nodes own a simplified
Fidget `Shape` and children. Native evaluation tapes are temporary and recycled.
The prototype uses scoped native workers with the same worker count as the
production Rayon pool; its scheduling is not otherwise identical to production.

## Results

Headless release builds on the development Apple Silicon Mac, 50% playback of
the actual 6,588-segment CAM example, 349 scene objects, 333 × 750 image, final
depth 3072 (the native depth is rounded to 768). Grap evaluation and construction
of the stock/path expressions precede all timed rendering sections. The
production progression includes its own scene preparation; prototype and
final-only times exclude the shared scene preparation (about 0.15 s here).
Add that preparation cost when comparing their complete request costs.

| Renderer | Time through final image | Peak process RSS |
| --- | ---: | ---: |
| Current production progression | 4.28 s | 1.19 GB |
| Retained spatial programs, strides 16/8/4/1 | 2.61 s | 4.43 GB |

These are exploratory observations, not a benchmark confidence interval. RSS
comes from separate warmed-build processes using `/usr/bin/time -l`; each also
renders a final-only reference before the measured progression. It includes
scene preparation, test infrastructure, scratch storage, and retained programs,
not just the new tree's footprint. The remaining excess memory has not been
fully attributed by an allocation profile.

For the final retained representation:

| Stride | Stage time | Cumulative time | New interval evaluations | Point samples |
| --- | ---: | ---: | ---: | ---: |
| 16 | 0.988 s | 1.000 s | 1,539,919 | 22,760 |
| 8 | 0.600 s | 1.605 s | 181,433 | 58,126 |
| 4 | 0.399 s | 2.008 s | 9,477 | 463,084 |
| 1 | 0.591 s | 2.605 s | 2,838 | 29,546,176 |

The final geometry matches production **exactly**, including depths, normals,
object indices, and ties; this is stronger than comparing shaded images. The
coarse images are much rougher than production's first 512-pixel-edge image,
so the approximately one-second first-image times are **not quality-matched**.
The final-only production render took 2.11 s in this run.

As an isolation control, initially rebuilding the same prototype's spatial
tree for each stride took 7.55 s versus 2.49 s retaining it. That is evidence
for reuse, not a claim of a 3× improvement over the current application.
The initial prototype retained every node's JIT tapes and peaked at 8.41 GB.
Recycling point/gradient tapes lowered this to 6.02 GB; also recycling interval
tapes lowered it to 4.61 GB. Retaining only `Shape` programs, without inherited
native tape references, gave the 4.43 GB / 2.61 s result above. These experiments
rule out simply keeping every existing `RenderHandle` alive as a practical design.

The last stage retains 1,747,022 region entries, including unvisited slots,
and 96,792 boundary programs. Nearly all interval work was done in earlier
stages: a fresh final traversal performs 1,733,667 interval evaluations.

## Bounded tile retention

The follow-up keeps one 64×64 image tile per worker alive through strides
16/8/4/1, publishing each result, then drops its spatial tree before claiming
the next tile. This is a bound on the **number of live tile trees**, not a byte
budget: scene complexity and depth still determine a tile's size. Output pixels
and lightweight root descriptors continue to scale with image dimensions.

| Mode | Image height | Workers | Render time | Peak process RSS |
| --- | ---: | ---: | ---: | ---: |
| Production independent passes | 750 | 11 | 4.28 s | 1.19 GB |
| Whole-view retained tree | 750 | 11 | 2.61 s | 4.43 GB |
| Bounded retained tiles | 750 | 11 | 2.54–2.59 s | 2.37–2.59 GB |
| Bounded retained tiles | 750 | 6 | 2.81 s | 2.00 GB |
| Production independent passes | 1500 | 11 | 8.61 s | 1.21 GB |
| Bounded retained tiles | 1500 | 11 | 5.22 s | 2.06 GB |
| Production final-only pass | 750 | 11 | 2.09 s | 1.06 GB |

The preparation caveat above applies to this table too. The 1500-pixel case
uses a correspondingly finer final voxel grid, including depth. Different
specializations and allocator high-water marks mean memory is not monotonic
with resolution; these measurements do not establish a general memory bound.

Bounded retention produces exactly the same final geometry and does the same
interval/sample work as whole-view retention. At 750 pixels it publishes 288
tile updates and performs 1,733,667 interval evaluations and 30,090,146 point
samples. At 1500 pixels it publishes 1,056 updates. There is no whole-view stage
barrier: a fine tile can appear before coarse work on a different tile finishes.

This exposes a simpler control: the current renderer already streams final
tiles, and CAM already has a current mesh underneath. A single final-quality
pass can therefore provide activity without retaining any new spatial tree.
Its 750-pixel render takes about 2.1 s (roughly 2.25 s including preparation),
versus about 2.7 s including preparation for bounded refinement. First-tile
times are not time-to-whole-image measurements and do not alone establish
perceived responsiveness.

Stock-specific timing confirms the early pixels are not just cheap toolpath
objects. In a paired run at 50% playback, final-only publishes its first stock
tile at 115 ms and finishes in 2.09 s; bounded refinement first shows stock at
135 ms and finishes in 2.57 s. With fully cut stock (100% playback), final-only
first shows stock at 409 ms and finishes in 3.68 s; bounded refinement first
shows stock at 642 ms and finishes in 4.22 s. Scene preparation is excluded
from these times (148 ms and 220 ms respectively). Final geometry is identical
in both comparisons. The full-stock bounded run peaks at 3.28 GB, illustrating
the scene-dependent memory cost even with only eleven active tile trees.

These are **first visible patches**, not a coarse image of the entire stock.
We have not measured coverage-percent milestones or visually compared the
two schedules in the app. Whole-view progression can still be preferable when
a few difficult tiles dominate completion or there is no useful mesh preview.

## Verification and reproduction

Twelve ordinary unit tests check:

- A thin sheet missed entirely by the coarse samples appears at full detail.
  Repeating the fine stage performs zero new interval evaluations.
- Ordered object ties, clipping/saturated pixels, normals, and partial edge
  tiles match production, including with exact-program sharing enabled.
  An already-cancelled request returns no result.
- Bounded and whole-view refinement produce identical geometry and work
  counts. Each clipped tile publishes its levels in order. Cancellation from
  a tile callback stops subsequent refinements and returns no completed image.
- The program encoding preserves signed zero, infinities, and distinct NaN
  payloads, and ignores map insertion order.
- Forced hash collisions cannot merge different programs or variable identities;
  independently compiled identical programs share the same underlying VM data.
- Subtree reconstruction finds shared branches across different expressions,
  undoing register renaming, copies, and spills without counting unused writes.
- Subtree keys preserve constant bits, variable identities (rather than argument
  slot numbers), and operand order.
- Real compiled programs with three versus 255 registers reconstruct to the
  same expression, produce the expected numeric results, and existing shared
  storage is counted only once.
- Persistent specialization reuses unchanged nodes and leaves older roots valid.
- Discarded branches do not produce unused rewritten nodes; reusable traversal
  scratch remains correct across separate tile arenas and counter wraparound.
- The persistent renderer matches flat-program geometry at **every** refinement
  level on a scene with intersecting surfaces, a thin feature, and object ties.
  Repeating its final stage adds no interval evaluations, and cancellation works.
- Dependency-order lowering avoids spills for an adversarially allocated chain
  of independent branches. Every binary operation round-trips with immediate
  constants, preserving operand order, signed zeros, infinities, and NaN bits.

The real CAM diagnostic additionally checks the final geometry against the
current JIT scene renderer and writes `cam-retained-{16,8,4,1}.png` into the
sandbox build directory. Its core kernel deliberately mirrors upstream's voxel
algorithm for comparison; it is not a second supported renderer.
Bounded mode writes `cam-bounded-final.png` and compares geometry too.

```sh
./tools/sandbox-cargo test --release -p progred --lib diagnostics::progressive --quiet

/usr/bin/time -l ./tools/sandbox-cargo test --release -p progred --lib \
  cam_render_profile --config 'env.CAM_RETAINED_REGIONS="retained"' \
  --config 'env.CAM_HEIGHT="750"' -- --ignored --nocapture

/usr/bin/time -l ./tools/sandbox-cargo test --release -p progred --lib \
  cam_render_profile --config 'env.CAM_RETAINED_REGIONS="baseline"' \
  --config 'env.CAM_HEIGHT="750"' -- --ignored --nocapture

/usr/bin/time -l ./tools/sandbox-cargo test --release -p progred --lib \
  cam_render_profile --config 'env.CAM_RETAINED_REGIONS="bounded"' \
  --config 'env.CAM_HEIGHT="750"' -- --ignored --nocapture
```

Use `fresh` for rebuilding the prototype per stage, or `1` for both fresh and
retained runs. Use `final` for just the existing final-quality reference pass.
`CAM_RETAINED_WORKERS` limits bounded-mode workers; `CAM_PROGRESS` changes the
playback fraction (default 0.5). Warm the build before measuring RSS; otherwise
compiler memory can dominate it. These settings exist only in the ignored test.

## Exact whole-program sharing

The large retained memory cost is not primarily pixels: the 333×750 output is
about 8 MB. Temporary instrumentation counted about 31 MB of region/boundary
entries before allocation overhead. More importantly, simplification allocated
76,633 smaller programs containing **143,460,240 register instructions**. At
eight bytes each, their register-tape contents alone occupy **1.15 GB**, before
capacity and the additional SSA tapes/metadata each program retains. Unchanged
programs already share their parent's reference-counted storage. These counts
explain substantial retention overhead, not the whole process's memory.

The follow-up experiment compares each newly specialized program against a
request-local pool and optionally reuses an exact match. Equality includes both
the register and SSA tapes, variable identities/mappings, and simplification
metadata. Floats compare by bits, not numeric equality. Hashes only select a
bucket; a full encoding comparison confirms a match. This is structural
identity, not algebraic equivalence or equality up to register renaming.

At 50% playback and height 750:

| Program count / payload | Total | Exact duplicates | Duplicate fraction |
| --- | ---: | ---: | ---: |
| Newly allocated specializations | 76,633 | 43,921 | 57.3% |
| Register instructions in those programs | 143,460,240 | 9,911,882 | 6.91% |

Thus 32,712 distinct encoded programs remain. There are many duplicates, but
they are mostly small; the large programs are largely distinct under this
comparison. Removing the duplicates eliminates about 79 MB of register-tape
contents, plus their SSA tapes, capacities, and other overhead.

Separate warmed-build runs of the same whole-view retained experiment:

| Mode | Time through final image | Peak process RSS |
| --- | ---: | ---: |
| No interning or duplicate measurement | 2.63 s | 4.43 GB |
| Measure duplicates, retain original programs | 3.28 s | 4.51 GB |
| Measure and share exact duplicate programs | 3.27 s | 4.19 GB |

All three produce **exactly the same final geometry** as production. These
single-run timings remain exploratory, not confidence intervals. Scene
preparation is excluded as above (about 0.14–0.16 s). The sharing implementation
saves about 237 MB versus the uninstrumented control, roughly 5.4% of process
peak RSS, while taking about 24% longer. Nearly all that time penalty is already
present when measuring without sharing.

The implementation is deliberately confined to diagnostics. It uses Fidget's
existing serialization interface and JIT-to-VM view, with no new dependency or
Fidget patch. It serializes into worker-local reusable scratch buffers, hashes
them, and compares matching candidates under one mutex. The pool retains shape
references, **not** another permanent copy of every encoded program. Encoding,
hashing, comparison, and synchronization all contribute overhead; they have not
been individually profiled or optimized. The pool lives for the whole request,
so this experiment disallows bounded-tile mode rather than quietly preventing
finished tiles from releasing their programs.

Reproduce sharing with:

```sh
/usr/bin/time -l ./tools/sandbox-cargo test --release -p progred --lib \
  cam_render_profile --config 'env.CAM_RETAINED_REGIONS="retained"' \
  --config 'env.CAM_SHARE_PROGRAMS="share"' \
  --config 'env.CAM_HEIGHT="750"' -- --ignored --nocapture
```

Use `CAM_SHARE_PROGRAMS="measure"` to count without substituting shared programs;
omit it for the uninstrumented control. Counts are cumulative at each stride.
This measurement does not assess sharing common *parts* of otherwise different
programs, nor quantify how much memory that larger representation change could
save. The next census measures that separately.

## Shared-subtree census

This is an **offline duplication check**, not a rendering implementation. After
the final render has matched production geometry, it visits the original scene
programs and every retained boundary program. Existing shared VM allocations
are counted only once. It reconstructs expression nodes from register
instructions, replacing registers and spill slots with expression identities.
Copies disappear, immediate constants become bit-preserving constant nodes,
and input slots resolve to their actual variable identities. It does not
reorder operands, reassociate arithmetic, or otherwise seek algebraic equality.

Nodes are interned by exact `(operation, children)` keys (or constant/variable
keys); ordinary hash-table equality confirms matches. A child identity already
represents its complete subtree, so this compares arbitrary-size shared
subexpressions without expanding them into trees. Each program's reachable
nodes are counted once, avoiding double-counting overlapping subtrees. A second
count first removes duplicate complete expression roots, showing the additional
opportunity from sharing *within different* expressions. An expression root is
not a complete executable tape with register allocation and trace metadata.

Same 750-pixel-high CAM view, two playback positions:

| Census | Half cut (50%) | Fully cut (100%) |
| --- | ---: | ---: |
| Original scene programs included | 349 | 3 |
| Original + retained program allocations | 76,982 | 69,715 |
| Distinct complete expression roots | 33,061 | 25,580 |
| Node occurrences, unique within each program | 115,334,793 | 172,251,386 |
| Node occurrences after whole-expression deduplication | 103,289,355 | 170,106,054 |
| **Unique nodes across every expression** | **3,632,968** | **6,251,753** |
| Shared-node payload, at the census's 24 bytes/node | 87.19 MB | 150.04 MB |
| One 4-byte output handle per program | 0.31 MB | 0.28 MB |
| Existing register-tape payload alone (8 bytes/instruction) | 1,150.58 MB | 1,860.09 MB |

These are decimal MB. The distinct nodes include inputs/constants/operations:

| Node kind | Half cut | Fully cut |
| --- | ---: | ---: |
| Inputs | 3 | 3 |
| Constants | 8,119 | 6,333 |
| Unary operations | 367,964 | 574,730 |
| Binary operations | 3,256,882 | 5,670,687 |

The repetition is overwhelmingly in arithmetic, not merely repeated constants
or coordinates. Even **after** deduplicating entire expression roots, sharing
subtrees removes 96.5% / 96.3% of the remaining node occurrences. Using the same
24-byte node representation independently per program would take 2.77 / 4.13 GB;
this is a representation comparison, **not** a measurement of the current tapes.

The 87 / 150 MB estimate is concrete graph payload, not a forecast of process
RSS. It excludes the interning index, allocator spare capacity, spatial trees,
per-region/root metadata, scratch storage, and compiled evaluation/JIT tapes.
For scale, the actual census's node-vector capacities were 100.66 / 201.33 MB,
and its interning maps had capacity for 3,670,016 / 7,340,032 entries. The census
also keeps bookkeeping sets that a real implementation need not retain.
Existing programs additionally contain SSA tapes, not included in the measured
register-tape payload. Replacing their long-lived copies with a shared graph
therefore has a substantial storage opportunity, but **keeping those tapes
alongside the graph would not realize the savings**.

The renderer itself is unchanged. The census took 18.62 / 33.87 seconds after
rendering; that deliberately unoptimized reconstruction cost says nothing about
constructing shared nodes directly during specialization. Both runs checked
exact final depths/normals/object precedence against production first. Process
peaks of 4.71 / 6.16 GB include *both* the original retained programs and the
extra census graph, so they are not shared-renderer memory measurements.

Reproduce with:

```sh
/usr/bin/time -l ./tools/sandbox-cargo test --release -p progred --lib \
  cam_render_profile --config 'env.CAM_RETAINED_REGIONS="retained"' \
  --config 'env.CAM_SUBTREES="1"' --config 'env.CAM_HEIGHT="750"' \
  --config 'env.CAM_PROGRESS="0.5"' -- --ignored --nocapture
```

Use `CAM_PROGRESS="1.0"` for fully cut stock. The census requires whole-view
retention and disallows `CAM_SHARE_PROGRAMS` so the input allocations are not
deduplicated before counting. The census itself needed no production API or
vendored Fidget changes.

## Persistent-expression renderer

The follow-up is a real rendering prototype, not another census. It retains
immutable expression nodes and one root ID per boundary region. A scene-wide
base arena contains original expressions; each image tile has its own arena
for rewritten nodes, looking in the base before allocating. Specialization
only walks branches selected by the interval trace and reuses unchanged nodes.
Earlier roots remain valid. All tile arenas survive through the whole-view
refinement sequence, then are dropped with the request.

This is deliberately simpler than a globally synchronized interning table or
per-node reference counting. It shares the base across workers but **does not
deduplicate new nodes between tiles**. Consequently it cannot reach the global
census's ideal node count. It also retains nodes that later versions no longer
use until their tile arena is dropped. No algebraic reassociation is performed:
constant bits, variable identities, operation kinds, and operand order identify
nodes, just as in the census.

When a region needs evaluation, its reachable graph is flattened to SSA,
register allocated, and wrapped as a temporary Fidget function. Existing JIT
evaluators consume that function. The compiled tape and native evaluation
tapes are not retained per region; native executable storage is recycled.
Original scene-root interval tapes remain prepared, as in production. No graph
interpreter or per-pixel pointer chasing was introduced.

Three additive APIs in the pinned `fidget-core` support this experiment:
borrowing a function's SSA tape, lowering a valid SSA tape with its variable
map, and wrapping a function as a shape. See the standalone
[patch](experiments/fidget-core-ssa.patch). The package is copied at the same
pinned revision as the already-vendored JIT/raster packages; there is no
dependency upgrade or change to existing evaluator behavior. One additional
upstream-package regression test checks the new SSA entry point against the
existing lowering path.

### Measurements

Same CAM scene, camera, image size, strides, and 11 workers as above. Final
geometry matches the production reference exactly in both playback positions.
Timings exclude shared scene preparation (0.15 s half cut, 0.22 s fully cut)
but include graph import and root re-lowering. RSS remains whole-process peak,
including the preceding production final-only reference render.

| Representation | Playback | Time through final image | Peak process RSS |
| --- | --- | ---: | ---: |
| Retained flat programs, paired control | Half cut | 2.53 s | 4.43 GB |
| Persistent DAG, indexed traversal scratch | Half cut | 6.18 s | 1.62 GB |
| Retained flat programs, paired control | Fully cut | 4.30 s | 6.28 GB |
| Persistent DAG, indexed traversal scratch | Fully cut | 9.36 s | 2.20 GB |

The half-cut persistent graph contains 5,382,260 nodes with 175.23 MB of node
vector capacity; fully cut contains 9,130,750 nodes with 292.66 MB of capacity.
Those capacities **exclude** interning tables, spatial trees, worker scratch,
original programs, and transient compilation/evaluation storage. They explain
part of the process memory, not all of it. The global census was 3.63 / 6.25
million nodes, versus this deliberately per-tile representation's 5.38 / 9.13
million.

The first unoptimized persistent prototype took 13.75 s / 1.99 GB on half cut.
Avoiding reconstruction of discarded branches reduced that to 12.07 s /
1.60 GB. Replacing hash-based traversal bookkeeping with per-worker indexed
scratch reduced it to 6.18 s / 1.62 GB. Scratch uses traversal stamps so values
are never reused as computation results across traversals or tile arenas.
The interning index remains an ordinary hash table.

The remaining cost is real: a frame makes 249,685 temporary program lowerings
at half cut, or 490,783 fully cut, including revisits at finer levels. Diagnostic
timers identify graph walking/flattening, specialization, and register
allocation as significant work. These counters sum elapsed worker time and
**must not be read as wall-clock phase durations**. This first lowerer emitted
register operations even for constants, and scheduled instructions in arena
allocation order. The follow-up below removes those two inefficiencies.

```sh
/usr/bin/time -l ./tools/sandbox-cargo test --release -p progred --lib \
  cam_render_profile --config 'env.CAM_RETAINED_REGIONS="dag"' \
  --config 'env.CAM_HEIGHT="750"' --config 'env.CAM_PROGRESS="0.5"' \
  -- --ignored --nocapture
```

Use `CAM_PROGRESS="1.0"` for fully cut stock. This mode uses whole-view retention;
it does not combine with the separate whole-program interning or subtree census.

Initial verification: all 11 experiment tests passed; the full Progred library
suite passed 680 tests (31 ignored diagnostics), and the vendored Fidget core,
JIT, and raster suites passed 265, 196, and 19 tests respectively. The core patch
also applies cleanly in a dry run against the pinned upstream checkout.

## Lowering follow-up — 2026-09-16

This pass keeps the persistent representation and temporary-tape lifetimes
unchanged. It adds instruction-count diagnostics and makes two compiler changes:

- Visit dependencies in depth-first, left-to-right order, assigning registers
  as their results become available. Do not sort reachable nodes by allocation
  identity. That sorting was both extra work and a source of unnecessarily long
  live ranges, hence register spills. Left-first traversal also avoids retaining
  every right-hand leaf of a left-associated chain. This is a simple schedule,
  not a general register-pressure optimizer, and it does not reassociate math.
- Use Fidget's right-immediate instruction when a binary operation's right
  operand is constant. Such a constant needs no separate register definition
  unless another use requires one. All binary operations have this form; operand
  order and float bits remain unchanged. Left-hand constants remain registers
  rather than introducing swaps or more special cases.

| Same persistent-DAG renderer | Half-cut time | Fully-cut time | Half / full peak RSS |
| --- | ---: | ---: | ---: |
| Before this pass | 6.18 s | 9.36 s | 1.62 / 2.20 GB |
| Dependency order + immediate constants | 4.66 s | 7.00 s | 1.61 / 1.98 GB |

Timing/preparation/RSS caveats above still apply. These are exploratory runs,
not confidence intervals. Both real CAM runs matched production's complete
depth/normal/object result exactly, and retained the same region/graph counts
and performed the same interval and point-sample work. They still make 249,685 /
490,783 lowerings; the compiler simply does less work per lowering.

For half cut, the diagnostic's total SSA instruction count dropped from
299,493,341 to 253,685,340, and register-tape instructions from **610,227,963 to
289,165,868**. These count temporary programs each time they are compiled, not
per-pixel executed instructions or unique retained nodes. The difference between
SSA and register counts includes register copies and spills. The new graph has
no additional retained metadata or program cache.

Isolation experiments, deliberately **not retained**:

- Borrowing the parent's compiled program when a child keeps the identical
  root removed about 22,000 lowerings, but only tiny programs. With the earlier
  dependency-order implementation it measured 5.15 s versus 5.14 s without it;
  the extra recursive borrowing plumbing was removed.
- A one-byte-per-node annotation let specialization skip subtrees without
  choice operations. It reduced some specialization work, but added graph
  construction/bookkeeping and did not improve overall time: 4.68 / 7.32 s with
  it versus 4.66 / 7.00 s without. Node/annotation vector capacities increased
  from 175.23 / 292.66 MB to 182.53 / 304.85 MB. It was removed too. The samples
  do not establish that it can never help a different workload.

The intermediate right-first dependency schedule alone took 5.14 s at half cut;
adding immediate constants took 4.63 s. The retained left-first schedule is
within that timing range and additionally handles the adversarial-chain test
without spills. It is not universally optimal for arbitrary expression DAGs.

The same reproduction commands select the revised implementation. All changes
in this pass are in the test-only experiment; there are no further vendored
Fidget changes or app-renderer changes.
Verification after this pass: all 12 experiment tests pass; the complete Progred
library suite passes 681 tests with 31 ignored diagnostics. Both CAM cases also
pass their exact production-geometry comparison. `git diff --check` is clean.

## Decision / next experiment

The reuse principle is sound and worthwhile; the unbounded whole-view retained
tree is **not ready to replace production**. Keep the current renderer for now.

Exact whole-program sharing as implemented here is also **not worth promoting
to production**: modest memory savings do not justify its measured runtime
overhead. Retain the diagnostic for reproducibility, not as application policy.

Bounded retention substantially reduces memory, but does not yet justify a
second rendering kernel over streaming a single final-quality pass. For this
mesh-backed CAM view, the simpler next experiment is **mesh → final implicit
tiles**. That scheduling change was left out of the retained-renderer experiments
and subsequently implemented as the app trial below. Standalone implicit
previews without a mesh may still need their coarse passes.

These observations do not rule out genuine whole-view progressive refinement;
it provides a different intermediate result. The persistent-expression follow-up
now demonstrates substantial **actual memory savings**. The lowering cleanup
cuts its render time by about a quarter without surrendering those savings,
but it is still slower than the flat-program prototype and slightly slower than
the earlier production-progression measurement at half cut. Do not promote it
to production in this form. An efficient representation/lowering boundary, or
an explicitly bounded lifetime for some compiled programs, would need another
measured experiment; retaining every tape would surrender the savings.

The current experiments do not retain point samples or add world-space reuse
across camera/scene changes.

## App trial: mesh → final implicit tiles

`preview paths refined` now requests one native-XY, four-times-depth implicit pass.
It skips the independent coarse-XY and native-depth passes, keeping the current
mesh underneath unfinished regions. Existing coverage-aware tile publication,
progress reporting, mesh readiness, cancellation, and latest-request scheduling
are unchanged. The bar covers one pass rather than restarting between levels.

This uses the existing production raster kernel, not a retained renderer or the
new core SSA APIs, and requires no further Fidget patch. `Passes::Progressive`
retains the former sequence; setting its `first_max_edge` to 512 in the refined
composition restores the old comparison. Standalone implicit previews still use
their 128-pixel-start progression. No user-facing mode switch was added.

The new regression tests compare final pixels against the multi-pass reference,
check one progress interval, and exercise cancellation and invalid depth. The
visual tradeoff remains for in-app testing: regions become final-quality over
the mesh rather than the whole image improving through intermediate resolutions.
Verification: 683 Progred library tests pass, with 31 ignored diagnostics;
`git diff --check` is clean. The app was not launched for this change.
