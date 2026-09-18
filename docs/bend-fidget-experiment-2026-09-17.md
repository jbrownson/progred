# Bend 2 / Fidget interval experiment — 2026-09-17

The limited port works, including repeated and concurrent use of one runtime-loaded
program. It does **not** improve performance on this machine. Bend's CPU evaluator
scales about 3.5× from one to eight workers, but remains slower than Fidget's VM.
The best Bend CPU and Metal configurations are roughly 11–25× slower than the
existing Fidget JIT with eight workers. Keep Fidget as the production evaluator.

## Scope and environment

- Apple M3 Pro, 11 CPU cores, 36 GiB RAM; macOS 27.0, build 26A428.
- Bend CLI 2.0.5, pinned to
  [`46df6bef271702221dafac6c85dfb36012dd0ef1`](https://github.com/bendlang/bend/tree/46df6bef271702221dafac6c85dfb36012dd0ef1).
  Node 26.4.0 and Apple clang 21.0.0; native C built with `-O3 -ffp-contract=off`.
  Bend's Metal compiler uses its upstream safe math setting.
- Progred HEAD `9915631f405b2b9d64e414d756288393d3904228`, with the existing
  uncommitted Grap/toolpath work present. Comparisons use exactly the same
  exported fixtures; they should not be compared directly with older CAM reports.
- The actual `ball_path` example supplies stock fields at 0.2%, 2%, and 100%
  playback. The tapes contain 209, 1,610, and 74,559 register instructions;
  the largest includes 6,237 stores and 6,237 loads for spills.
- Interval queries cover a deterministic permutation of a 16³ spatial grid.
  Every seventh query has zero width; the others have radius 1/128 per axis.
  Small and medium evaluate 4,096 samples; large evaluates 1,024 samples.

This is an interpreter for the CAM tapes' arithmetic subset, not a full Fidget
port. Export rejects unsupported instructions. It preserves instruction order,
operand order, register reuse, shared subexpressions, and spill locations.
It does not implement tape simplification, gradients, adaptive traversal, or
rendering. It tests parallel samples, not parallel dependencies inside one
expression. Bend cannot discover those dependencies in this sequential tape.

## Results

Milliseconds per complete sample batch, median of three measured rounds after
one warm-up. Compilation, loading, and result comparison are outside these
times. Query generation, workspace allocation, result construction, and branch
hashing are inside them. Native workers are started and joined per batch; Bend
uses its existing pool, so that difference favors Bend.

| Tape | Samples | Fidget VM, 8 workers | Fidget JIT, 8 workers | Bend CPU, 8 workers | Bend Metal |
|---|---:|---:|---:|---:|---:|
| Small | 4,096 | 0.615 | **0.243** | 2.938 | 2.735 |
| Medium | 4,096 | 3.687 | **0.816** | 20.714 | 17.043 |
| Large | 1,024 | 36.114 | **17.805** | 224.278 | 238.904 |

Bend CPU uses the better of the tested fork depths: depth 6, or 64 leaf jobs.
The best GPU configurations use one sample per leaf: depth 12 for small/medium,
depth 10 for large. This deliberately gives Bend its best measured configuration.

| Tape | Bend CPU, 1 worker / 64 jobs | Bend CPU, 8 workers / 64 jobs | CPU speedup |
|---|---:|---:|---:|
| Small | 10.203 | 2.938 | 3.47× |
| Medium | 72.538 | 20.714 | 3.50× |
| Large | 768.487 | 224.278 | 3.43× |

Task size matters strongly on Metal:

| Tape | 64 leaf jobs | 1,024 leaf jobs | 4,096 leaf jobs |
|---|---:|---:|---:|
| Small | 43.830 | 7.038 | 2.735 |
| Medium | 334.869 | 33.236 | 17.043 |
| Large | 3,630.681 | 238.904 | — |

The GPU improvement with more jobs is substantial, but it does not close the
gap with Fidget. These results do not establish that every possible Bend
representation would be slow; they reject this straightforward, reusable tape
interpreter as a performance improvement.

## Reuse, correctness, and representation

One immutable `Tape` value is loaded once per process. Every batch shares it
across workers; each leaf owns an `Array<Ival>` workspace and reuses that array
across its samples. Four consecutive batches reuse the same tape. The once-only
closure restriction is therefore **not a blocker**: callable code is a top-level
function, and the reusable Fidget program is data.

All 20 final Bend configurations completed successfully: 253,952 interval
evaluations across their warm-up and measured rounds. Every returned lower and
upper bound matched Fidget bitwise, and every per-sample branch-choice hash
matched. The comparator permits differing NaN payloads. Hash matching is a
diagnostic check, not proof that every individual choice is equal, and this is
not exhaustive floating-point validation. Native JIT results also matched the VM.

The interpreter was checked by Bend without `@unsafe`. That establishes Bend's
type/usage/termination checks for the Bend code, not a proof of Fidget semantics,
floating-point enclosure, or correctness of the C loader/runtime.

An initial version allocated closures at each instruction and took about 41 ms
for the small single-worker batch. Ordinary helper functions and a tail-recursive
state parameter removed those allocations, reducing that to about 10 ms. The
final generated evaluator borrows the shared tape while traversing it. Remaining
costs include dynamic instruction dispatch and a boxed linked tape representation;
this experiment did not separately profile those costs.

## Boundary costs and memory

The binary input sizes are 52,524 / 74,940 / 1,205,260 bytes, including reference
outputs. The Python JSON-to-binary diagnostic bridge took approximately 1.4–1.8 /
2.4–3.3 / 52–62 ms. This is not a prediction of an optimized direct Rust encoder.
Constructing Bend's tape from that binary took roughly 0.01–0.09 / 0.03–0.13 /
0.67–0.86 ms, outside evaluation timing.

The final CPU executable took approximately 0.78 s to type-check, emit C, and
compile on a warm toolchain. Its source is independent of the loaded tape.
Fidget's measured additional JIT interval-tape construction, after creating its
shape, took 0.046 / 0.081 / 4.761 ms. These are separate stages, not equivalent
end-to-end cold-start measurements.

The best eight-worker Bend CPU processes reached about 4.1 / 4.2 / 10.1 MiB
maximum resident memory. The best Metal processes reported about 25.8 / 26.4 /
25.4 MiB, **with a separately requested 1 GiB shared GPU heap**. Process RSS is
not a complete measure of GPU memory. A 256 MiB heap was rejected by Bend because
its fixed rings, stacks, and per-lane pages did not fit. Large virtual address
reservations on CPU likewise must not be mistaken for resident memory.

Metal compilation/startup is outside the timed batches but included in the raw
whole-process measurements. The signed benchmark has no writable archive beside
its executable, so it requests compilation at each launch; Apple's own compiler
cache can make subsequent launches cheaper. These are not pristine cold-cache
measurements.

## Reproduction and changes

See [the harness instructions](../tools/bend-fidget/README.md) and
[retained raw results](../tools/bend-fidget/results-2026-09-17.json), which include
source/fixture hashes, all timing rounds, and the native diagnostic output.

Rust builds and CPU runs use the repository's Seatbelt boundary. Metal runs use
a separate signed, headless bundle with only the standard App Sandbox entitlement;
no network, user-file, or JIT permission is added. The Progred app was not run.

The only existing source-file change registers an ignored test diagnostic. The
new interpreter, bridge, runner, and report are opt-in experiment files. No
production integration, dependency change, or commit was made.
