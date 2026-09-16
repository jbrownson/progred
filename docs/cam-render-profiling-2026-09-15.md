# CAM rendering investigation — 2026-09-15

## Scope and conclusion

Profiled the current `toolpaths.gid`: both operations, tilted ball cuts and the
new composed square-mill chamfer passes. This includes the pending height-based
camera framing. The program emits 6,588 segments. These are optimized,
headless measurements on the development Mac, not native input-to-display timings.
The first measurements below did not change production rendering. The later
follow-ups fix two upstream issues and integrate JIT for implicit rendering on
Apple Silicon macOS, while retaining VM meshing. See the final section for the
current configuration and [the explicit local patches](../vendor/README.md).

The dominant work is Fidget's CPU **VM evaluation**, especially interval
evaluation. It is not Grap running the program twice or expensive tree
construction. Fidget's existing CPU JIT is about twice as fast on the full stock
after fixing address-size limits in its Apple Silicon code generator and using
its recommended raster tiles. GPU spilling remains a separate unsolved issue.

## Reproduction

The ignored `cam_render_profile` test records `preview_operations` from the
example and applies its actual per-path tools to a one-inch stock cube. It uses
profile tolerance 0.001, path radius 0.005, camera yaw 30° / pitch 60° / zoom 1,
bounds ±0.85, and the production implicit renderer. The diagnostic separates
stock, future paths, and the current tool; production depth-composites these.

```sh
./tools/sandbox-cargo test --release -p progred --lib cam_render_profile \
  --config 'env.CAM_PROGRESS="0.5"' --config 'env.CAM_MESH="1"' \
  -- --ignored --nocapture
```

`CAM_HEIGHT` sets pixel height (default 750, width 4/9 of height). Cargo's
`--config env…` is intentional: the build wrapper clears incoming environment
variables. Optional diagnostic comparisons:

- `CAM_GROUPS=1`: combine future paths into batches of 1, 8, 32, or all paths.
- `CAM_STOCK_AB=1`: compare the production left fold with balanced and spatially
  balanced subtraction expressions; requires positive playback progress.
- `CAM_TILES=1`: compare root-tile sizes through Fidget's public setting.
- `CAM_MESH=1`: also measure depth-6 stock meshing.
- `CAM_JIT=1`: compare VM/JIT image and mesh results and cancellation latency
  (Apple Silicon macOS); add `CAM_JIT_DEFAULT_TILES=1` for production JIT tiles.

These controls belong only to the ignored test, not the application.

## Where the time goes

At **333 × 750**, production `[32,16,8]` tiles, with a 167 × 375 first image,
native pixels, then native pixels with 4× depth:

| Work | 50% playback | 100% playback |
| --- | ---: | ---: |
| Parse/setup + Grap path recording | 328 ms | 328 ms |
| Construct stock expression | 5.6 ms | 9.7 ms |
| Compile stock VM | 110 ms | 202 ms |
| Stock, first image (includes compilation) | 1.44 s | 2.45 s |
| Stock, native image | 2.45 s | 4.02 s |
| Stock, final 4×-depth image | 4.61 s | 7.55 s |
| Stock, entire refinement sequence | 8.50 s | 14.01 s |
| Future paths, entire sequence | 3.21 s | none |
| Current tool, entire sequence | 7 ms | 356 ms |
| Stock mesh, depth 6 (including compilation) | 1.27 s | 2.09 s |

The stock programs contain 287,393 / 463,702 VM instructions respectively,
including 68,972 / 121,212 register-spill loads and stores. Compilation is shared
between resolution passes within one image job.

A repeat of the complete-stock sequence measured 14.36 seconds (vs 14.01),
including 7.92 seconds for the last pass (vs 7.55). All 6,588 recorded segments
were distinct by endpoints and tool axis, even ignoring tool identity, and none
collapsed to zero length when converting their endpoints to f32. This rules
out exact duplicated/degenerate moves, not every possible geometric redundancy.

At **667 × 1500**, complete stock took approximately 49 seconds across all
refinements, including 30.5 seconds for the final depth pass alone. The first
five seconds of that run were CPU-sampled, so it is not a clean wall-time
comparison; the later passes ran after sampling stopped. This demonstrates the
size of the Retina-resolution cost, not a stable latency guarantee.

The five-second sample's leading active leaf stacks were interval evaluation
(19,223 samples), bulk float evaluation (6,302), tape simplification (1,092), and
gradient evaluation (368). Allocation was not a leading cost. These are CPU
samples across workers, not wall-time percentages. Sample output is in the
ignored `target/sandbox/build/cam-cpu-sample.txt`.

### Scheduling and reuse

The existing `editor_toolpath_refined_svg_captures` test passed over the full
editor pipeline. It verifies two initial jobs (mesh and implicit), only one
implicit job after orbit, no restart when a refinement is published, and two
new jobs when playback changes. The unit fixture also checks one shared Grap
evaluation and conflation of successive camera requests.

The full editor's refinement-publication frames were about 7–12 ms; its
headless mesh-fallback orbit frame was 36 ms. That last measurement uses CPU
triangle rasterization because the sandbox exposes no Metal adapter, unlike
the native app's triangle renderer.

Some work *is* repeated: mesh and implicit independently construct/compile the
stock, and a camera-only implicit request reconstructs and compiles its scene.
That is a small fraction of these runs. It could later become a shared,
camera-independent node in the existing computation graph; it does not justify
a separate ad hoc cache.

Each progressive resolution really reruns rasterization. No geometry work from
the coarse pass currently accelerates the finer one. The default larger scene
still benefits from the first mesh while those passes run.

## Experiments that did not help

At 50% playback, 347 future-path objects compile to 61,922 instructions with no
spill operations. Rendering native pixels separately took 0.91 s. Batches of
8 took 1.22 s; batches of 32 took 1.23 s; one combined field took 1.65 s.
Coverage was identical, but unioning changes some intersection normals/colors
(7 / 46 / 144 pixels respectively). Fewer render calls is not automatically
less work for this evaluator.

The stock's production left-fold subtraction took 2.44 s for a native image.
Balancing its expression took 4.23 s; spatially balancing by segment midpoint
took 4.55 s. Both matched the original image exactly, and reduced instruction
counts somewhat, but were slower. Depth-6 meshing also worsened from 1.33 s to
1.58 / 1.63 s. A shorter or better-balanced tape alone is not the objective.

For complete stock, root tiles 32 / 64 / 128 / 256 took 4.23 / 3.97 / 5.57 /
13.72 seconds with identical geometry pixels. The small 64 advantage is not
enough evidence to change a setting chosen partly for cancellation latency;
the large tiles are clearly not a solution here.

## JIT experiment and a concrete upstream limit

Temporarily enabled the pinned Fidget revision's existing `jit` feature. Its
additional dependencies were resolved through the repository wrapper's seven-day
minimum-age rule and built under Seatbelt. Dependency files were restored
after that initial experiment; the later integration is described below.

At 10% playback, native stock rendering measured **779 ms with VM vs 258 ms
with JIT**, with byte-identical shaded images. Shape construction was about
21 ms for either backend; the JIT rendering measurement includes demand-time
machine-code generation but excludes final shading (included in the VM number).
Its 4×-depth pass took 519 ms and depth-6 meshing took 128 ms. These are smaller
stock-program measurements, not an established 3× speedup for the full scene.

Larger programs fail during JIT generation:

- 35% playback: `aarch64/interval.rs:118`, `sp_offset <= 32768` assertion.
- 100% playback: `lib.rs:368`, `invalid mem offset: 151344 is too large`.

The pinned generator handles stack adjustments only below 65,536 bytes, and
interval spill loads/stores use limited-range immediate addressing. The vector
evaluators have related address checks. A proper fix needs large stack-frame
adjustments **and** valid large-offset loads/stores, with ABI/stack and all
evaluator tests—not merely deleting assertions. The temporary comparison
function was initially kept in ignored build output and has since been replaced
by the checked-in comparison linked below.

This is a more focused potential Fidget fork than redesigning rendering. We
should first reproduce it with a small standalone upstream test, then evaluate
a correct fix and benchmark the full scene. Enabling JIT in the packaged app
also needs an explicit signing/entitlement and platform-support review; the
headless test does not establish that integration.

The existing GPU obstacle is separate: the pinned GPU interpreter's `OP_MEM`
case is explicitly unimplemented. These stock tapes contain many such spill
operations. GPU support would require a correct memory/spill interpretation
through evaluation and simplification, not just changing an 8-bit counter.
No GPU speedup for this scene was measured.

## Integration decision after the initial measurements

The address-limit investigation is complete below. JIT **with its own recommended
tiling** roughly halves full-stock implicit render time; with Progred's smaller
tiles it does not materially improve the final refinement. Larger tiles worsen
cancellation latency. The follow-up below addresses cancellation inside root
tiles and integrates JIT only for Apple Silicon macOS implicit rendering,
retaining VM meshing and other platforms. Retaining more
spatial specialization work between refinements remains another upstream lead,
not a new Progred cache. GPU spilling is separate and unimplemented.

## Follow-up: local AArch64 JIT fix

The isolated checkout is `target/sandbox/fidget-jit-investigation`. The
[upstream-source patch](experiments/fidget-jit-aarch64.patch) is retained outside
that ignored directory so cleaning build output cannot lose the work. It is
against the same pinned revision. It was initially isolated; the integration
below now uses it from the checked-in vendor directory.

The patch fixes three issues:

1. Stack adjustment and spill accesses cannot encode larger immediate offsets.
   Small spills retain the original single instruction; large ones use a
   materialized offset and register-indexed addressing in all four evaluators.
2. Existing stack frames of at least 4 KiB clobber the callee-saved `x28` register.
   Address scratch now uses caller-saved `x9`. A protected assembly test caller
   demonstrates the violation without letting it corrupt the Rust test runner.
3. Float/gradient bulk loops have a conditional exit branch with a ±1 MiB reach.
   The complete stock's mesher exceeded that after the spill fix. The conditional
   branch now skips a nearby unconditional exit branch, which has greater reach.

The two new tests fail against unpatched upstream (register corruption and
impossible branch relocation) and pass with the patch. The complete JIT suite
passes: **196 tests**. Boundary coverage includes 4/16/32/64 KiB and 256 KiB
frames, all four spill formats, calls with live spills, loop bodies over 1 MiB,
empty batches, and repeated SIMD iterations.

These are Apple Silicon/macOS results. The patch is a candidate for upstream
review, not a certification of Windows/Linux stack growth, all platforms' ABIs,
or arbitrarily large tapes. Ordinary thread-stack limits and unconditional
branch reach still exist. No code was published or pushed upstream.

### Correctness and timings

At 333 × 750, comparing identical scene, quality inputs, and Progred's
`[32,16,8]` tiles:

| Work | Half stock VM | Half stock JIT | Full stock VM | Full stock JIT |
| --- | ---: | ---: | ---: | ---: |
| Native depth | 2.54 s | 1.23 s | 4.41 s | 3.01 s |
| Final 4× depth | 4.94 s | 3.07 s | 8.22 s | 8.08 s |
| Depth-6 mesh | 1.29 s | 1.18 s | 2.05 s | 2.32 s |

Rendering timings include demand-time code generation and shading, but exclude
the initial shape construction, which was ~105–110 ms at half stock and
~190–200 ms at full stock for either backend. Mesh timings exclude that same
initial construction. Half-stock timings include lightweight diagnostic counters;
full-stock table timings do not. An instrumented full-stock repeat measured
4.17/2.90 s native, 7.93/7.95 s final, and 1.99/2.37 s mesh (VM/JIT).

Both native and final shaded images were **byte-identical** between backends.
The mesh vertex arrays and triangle arrays were also identical: 10,632 vertices
at half stock, 16,952 at full stock. This establishes those fixtures, not universal
numerical equivalence for every possible expression.

#### Important: use JIT's own tiling recommendation too

Fidget's `JitFunction::tile_sizes_3d()` recommends `[64,16,8]`. Leaving its
tile setting unspecified, while retaining Progred's ordinary `[32,16,8]` for
the VM baseline, changed the full-stock result substantially:

| Work | Production VM | JIT with recommended tiles |
| --- | ---: | ---: |
| Native depth | 4.11 s | 1.90 s |
| Final 4× depth | 7.76 s | 3.68 s |
| Depth-6 mesh (independent of raster tiles) | 1.83 s | 2.17 s |

Images and mesh arrays still matched exactly. The earlier same-tile experiment
was useful for isolating backend behavior, but **not sufficient for choosing a
backend**: JIT's specialization/code-generation cost changes the best granularity.
This is a promising implicit-rendering speedup, not a reason to use JIT meshing.
Cancellation response in this run rose to 478 ms for JIT versus 160 ms for VM
after the request was signalled. That responsiveness tradeoff needs attention
before making the configuration the application default.

A second run confirmed it: native 3.91/1.97 s, final 7.27/3.52 s, mesh
1.81/2.19 s (VM/JIT), with exact images and mesh arrays again. Cancellation
returned 170/474 ms after the signal. Thus the roughly 2× implicit speedup and
the cancellation tradeoff both repeated.

### Code generation and cancellation

Temporary counters around JIT code generation measured 14,247 generated tapes
for native full stock, 36,614 for final full stock, and 2,071 for meshing. Their
summed elapsed generation time across workers was about 4.94, 8.66, and 2.02
seconds respectively. These sums are **not wall-time percentages or CPU time**;
workers run concurrently. Fidget generates specialized tapes as regions simplify,
so this is not evidence that Progred is accidentally rerunning the same frame.
The root interval tape is already generated once before cloning render workers.

The diagnostic also summed returned mapping capacities, but those mappings can
be recycled and larger than the code written into them. Those totals are not
allocation traffic or peak memory and should not be presented as such. A clean
peak-memory comparison remains unmeasured. A late CPU sample caught only the
tail of a run and was not used to attribute final-refinement costs.

Cancelling 50 ms after beginning a native render returned about 73 ms later for
VM and 103 ms later for JIT at half stock; full stock took 183/161 ms. These are
single observations, not latency bounds. Both returned cancellation rather than
a completed image. Code generation has no cancellation parameter, and unpatched rendering
checks cancellation between root tiles, not every instruction; JIT does not
solve prompt cancellation by itself.

## Follow-up: cancellation and app integration

Raster cancellation used to be checked between whole root tiles. A root tile can
contain many smaller interval, float, and gradient evaluations. The
[raster patch](experiments/fidget-raster-cancellation.patch) passes the existing
cancellation observation into that recursion and returns `None` all the way out
when cancelled. A partially filled tile is not a completed image. The public API,
sampling grid, and geometry math are unchanged; 2D rendering gets the same fix.

On the full stock, with the patch applied to both backends:

| Work | VM (32/16/8 tiles) | JIT (recommended 64/16/8 tiles) |
| --- | ---: | ---: |
| Native depth | 4.68 s | 2.32 s |
| Final 4× depth | 8.98 s | 4.26 s |
| Depth-6 mesh | 2.13 s | 2.59 s |
| Response after cancellation signal | 2.1 ms | 3.1 ms |

Both images remained byte-identical and mesh arrays matched. Cancellation was
signalled 50 ms into a native render; these are observations, not upper bounds.
Absolute throughput varied between runs, so the timings do not isolate the small
cost of the added cancellation polls. The earlier unpatched JIT cancellation
measurements were 474–478 ms. A single in-flight tape compilation/evaluation is
still not interruptible; neither are shape construction or image shading.

A final repeat after the app build and cross-platform checks finished measured
4.48/2.41 s native, 8.71/4.67 s final, and 2.22/2.97 s mesh (VM/JIT).
Cancellation returned in 2.0/8.2 ms, and images and meshes matched exactly again.
An intervening run overlapped compilation and was excluded from throughput
comparisons. Thus this remains approximately a 2× raster gain, with no reason
to replace VM meshing. Web and iOS checks passed (existing menu dead-code warnings),
and the macOS bundle built and passed signature verification without launching
the editor.

Progred now uses JIT for software implicit rendering **only on Apple Silicon
macOS**, with Fidget's recommended tiles, while preserving VM meshing and the VM
backend elsewhere. No scheduling or caching changes were needed.
The two small crate patches are checked in under [vendor](../vendor/README.md);
they do not depend on a developer-local checkout in `target`.

The signed macOS bundle adds Apple's standard allow-JIT entitlement, retaining
App Sandbox and hardened runtime, without unsigned-executable-memory exceptions.
All 196 JIT tests passed both normally and in a minimal signed bundle with the
app's actual entitlements. An initial bare executable test could not initialize
App Sandbox because it lacked bundle metadata; providing a proper bundle fixed
the test harness, with no relaxation of the app's protections.

Verification: 661 Progred tests passed (31 intentionally ignored), plus 196 JIT
and 10 raster tests. The new raster tests cancel at the start, middle, and final
poll of the last tile and verify that cancellation unwinds without publishing
that tile. The existing AArch64 JIT regression tests retain their large-spill
and long-branch coverage. No GUI was launched.

### Repeat the checked-in comparison

The [comparison](../progred/src/libraries/fidget/raster/diagnostics/jit.rs) is
compiled only for tests on Apple Silicon macOS. No temporary feature, manifest,
or lockfile edits are needed:

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  --config 'env.CAM_PROGRESS="1"' --config 'env.CAM_JIT="1"' \
  --config 'env.CAM_JIT_DEFAULT_TILES="1"' \
  cam_render_profile -- --ignored --nocapture
./tools/sandbox-cargo test --release -p fidget-jit -p fidget-raster --lib
```

Omit `CAM_JIT_DEFAULT_TILES` to isolate backend differences with identical
32/16/8 tiles. The test asserts equal images and meshes and measures cancellation,
but contains no code-generation instrumentation. The normal application has no
profiling switches or console timing output.

## Follow-up: progress over finished scene tiles

The former implicit progress bar summed equally weighted root tiles for each
object. At half playback there are 349 objects, with the expensive stock last;
finishing hundreds of small paths made the bar appear nearly finished before
most stock work had run. Deeper subdivision counts would not fix that weighting.

Software scenes now render every object's contribution inside each image tile,
then count that tile's actual image pixels as finished. The denominator is
`width * height`, excluding tile padding, independent of the number of objects.
Expressions remain separate, with the same voxel evaluator, depth clamp, and
first-object-wins tie rule. Root interval tapes are prepared once per pass;
workers retain object-specific render handles while reusing evaluator scratch.
Single-object raster entry points retain their existing tile-count callback.

Compared both traversals on the same compiled scenes, alternating their order
over three pairs per quality setting. Resolution was 333 × 750 with the default
camera and JIT tiles, and the last pass had 4× depth. Every comparison asserted
byte-identical final RGBA, including color/depth composition. The reference
object-major traversal exists only in the diagnostic test.

| Scene / pass | Object-major median | Tile-major median |
| --- | ---: | ---: |
| Half playback, native depth | 1.604 s | 1.208 s |
| Half playback, final depth | 2.861 s | 2.112 s |
| Full playback, native depth | 2.183 s | 2.151 s |
| Full playback, final depth | 4.090 s | 4.044 s |

Half-playback times improved about 25–26%; full-playback differences are small
enough to treat as noise. These are headless measurements, not end-to-end app
latency. The scene traversal avoids a separate parallel dispatch and full-image
merge for each object; no claim is made about which cost explains the speedup.

In the middle half-playback final-pass pair, the old bar reached 99% at 640 ms
but finished at 2.90 s. Tile-major reached 25/50/75% at 236/478/1304 ms, and 99%
near completion at 2.095 s; image assembly/shading finished about 2.5 ms later.
Callbacks at the app boundary are throttled to 50 ms, so nearby milestones may
arrive together. Progress is exact finished area, not elapsed-time prediction:
uneven tile cost can still leave a slow tail. Compilation remains at zero, and
the finished pixels are not yet published as partial images within a pass.

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  --config 'env.CAM_SCENE_TILES="1"' --config 'env.CAM_PROGRESS="0.5"' \
  --config 'env.CAM_HEIGHT="750"' cam_render_profile -- --ignored --nocapture
```

Use `CAM_PROGRESS="1"` for the fully cut stock. Regression tests also exercise
serial/parallel rendering, equal-depth ties, clipping, independently bound
variables, empty scenes, cancellation, and exact edge-tile pixel counts.
