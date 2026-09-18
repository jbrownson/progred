# Limited Bend / Fidget experiment

Headless interval evaluation of real CAM stock tapes. This is an opt-in
diagnostic, not a renderer backend or a general Fidget port. No production
dependencies or behavior change. The Progred app is never launched.

The Rust diagnostic exports the actual register instructions (including spills),
then records reference interval bounds and a per-sample hash of min/max choices.
`prepare.py` translates those instructions without algebraic rewrites. The Bend
executable is compiled once and loads each tape as data through a small C effect.
One shared immutable tape feeds a balanced fork of independent samples. Each
leaf owns an array and reuses it across its samples. Affine closures are used
only as single-use continuations; the instruction loop creates none.

Supported opcodes are exactly the subset used by these CAM fixtures. Export
rejects other opcodes. Float operation order is retained. Bounds are compared
bitwise, allowing different NaN payloads; trace hashes are diagnostic checks,
not a proof of equality. This does not implement simplification, gradients,
adaptive tile traversal, meshing, or rendering. It exposes sample parallelism,
not parallel dependencies inside one tape.

## Reproduce on Apple Silicon macOS

Requires the existing Rust toolchain, Python 3, Node with TypeScript support
(tested with 26.4), and Apple clang with Metal support (tested with 21.0).
All outputs and downloaded sources stay under `target/sandbox/bend-fidget`;
fixtures are under `target/sandbox/build/bend-fidget`.

```sh
# Downloads four source files at a fixed commit; executes nothing.
python3 tools/bend-fidget/build.py --fetch

# Uses sandbox-cargo for compilation and Seatbelt for the headless diagnostic.
python3 tools/bend-fidget/reference.py

# Type-check and compile once, then feed different runtime tapes to that binary.
python3 tools/bend-fidget/build.py
python3 tools/bend-fidget/run.py --depths 3 6 --threads 1 8

# Builds and signs a separate headless App Sandbox executable; opens no window.
python3 tools/bend-fidget/build.py --gpu
python3 tools/bend-fidget/run.py --gpu --depths 6 10 12 --threads 8
```

Bend is pinned at `46df6bef271702221dafac6c85dfb36012dd0ef1`. The C loader uses
that version's constructor layout and seals shared child pointers as its runtime
requires. It is not a supported, stable Rust embedding API. Compiler builds and
CPU executions use `cargo-sandbox.sb`, with Homebrew readable for Node and its
libraries, no network, and writes restricted to `target/sandbox`. GPU execution
uses a signed bundle with only `com.apple.security.app-sandbox`; it communicates
through inherited input/output streams. Bend requires more than a 256 MiB heap
for its fixed GPU scheduler; this test requests 1 GiB. No sandbox exception,
network access, user-file permission, or JIT entitlement is granted to Bend.

## Reading results

`reference.log` contains separate tape preparation and VM/JIT evaluation times.
`cpu-results.json` and `gpu-results.json` retain every process's raw output,
return code, whole-process time, and maximum resident memory. Each suite replaces
its previous result file. Each process loads a tape once and evaluates it four
times. Round 0 is warm-up; compare medians of rounds 1–3. CPU and GPU suites
should run sequentially with the reference benchmark, never simultaneously.

Kernel timing includes query generation, worker-local workspace initialization,
result materialization, and trace hashing. Native timings include starting and
joining worker threads; Bend uses a resident worker pool. Bounds/hash comparison
is outside timing. Conversion, loading, native compilation, and GPU startup are
reported separately. The GPU process time includes Metal compilation because
the sandboxed executable has no writable archive beside its binary.

All timed evaluators execute the full unspecialized root tape. A production
Fidget renderer usually simplifies child tapes; these kernel results alone do
not establish end-to-end rendering performance.
