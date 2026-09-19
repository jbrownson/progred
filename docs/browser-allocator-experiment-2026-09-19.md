# Browser allocator experiment — 2026-09-19

Follow-up to the [orbit profiling](browser-orbit-profile-2026-09-19.md). These
are isolated Chrome diagnostics, not changes to the normal application build.

## Candidates

1. Default Rust allocator: the pinned nightly's dlmalloc 0.2.14, protected by
   the standard library's global test-and-set spin lock.
2. Talc 5.1.1, using its documented threaded-WASM combination:
   `TalcLock<spinning_top::RawSpinlock, WasmGrowAndClaim, WasmBinning>`.
3. The same dlmalloc 0.2.14 through a small `GlobalAlloc` adapter, protected by
   `spinning_top` 0.3.0. Its lock first reads the lock word while waiting,
   instead of repeatedly writing it with atomic swap. Allocation, zeroed
   allocation, reallocation, and deallocation all use the same lock.

Both candidates still serialize allocation. Neither introduces thread-local
heaps or changes Progred's computation, invalidation, or scheduling semantics.
The second comparison keeps the allocation algorithm/version fixed, but is not
a bit-identical compilation differing in just one instruction: the standalone
crate and adapter have different code-generation boundaries from Rust's std.

Talc and spinning_top were resolved through the repository's seven-day-age
policy. Talc 5.1.1 was published September 9; the older 5.0.4 shown by cached web
documentation was not used. The candidate packages have no build scripts or
procedural macros. Builds ran under the ordinary dependency-code sandbox.

Primary references:

- [Talc's threaded-WASM setup](https://github.com/SFBdragon/talc/blob/master/talc/README_WASM.md#global-allocator-for-threaded-webassembly)
- [Talc 5.1.0 memory-growth change](https://github.com/SFBdragon/talc/blob/master/CHANGELOG.md)
- [spinning_top's lock implementation](https://github.com/rust-osdev/spinning_top/blob/v0.3.0/src/spinlock.rs)
- [dlmalloc Rust implementation](https://github.com/alexcrichton/dlmalloc-rs)

## Measurements

Same real editor-frame replay as the prior report: 2400×1800 physical pixels,
scale 2, half-width preview, expanded source, stock view, eight rendering workers.
Chrome 153.0.8010.48, Apple M3 Pro. Each trial starts a fresh isolated browser,
finishes the initial render, warms eight orbit frames, then measures 80 frames.
CPU profiling and screenshot capture happen separately from those timings.

At 2% playback:

| Allocator | Median CPU submission | p95 | WASM memory before orbit | After orbit |
| --- | ---: | ---: | ---: | ---: |
| Default | 23.38 ms | 24.48 ms | 204.38 MiB | 204.38 MiB |
| Talc | 22.21 ms | 24.14 ms | 201.81 MiB | 558.44 MiB |
| dlmalloc + alternate lock | 21.99 ms | 23.28 ms | 204.25 MiB | 204.25 MiB |

At 50% playback:

| Allocator | Mean CPU submission | Median | p95 | WASM memory before / after orbit |
| --- | ---: | ---: | ---: | ---: |
| Default, fresh comparison | 53.40 ms | 69.38 ms | 73.51 ms | 239.63 / 239.63 MiB |
| dlmalloc + alternate lock | 55.17 ms | 38.95 ms | 84.60 ms | 245.81 / 245.81 MiB |
| dlmalloc + alternate lock, repeat | 54.67 ms | 38.42 ms | 82.33 ms | 235.00 / 235.00 MiB |

The midpoint median is misleading. The default settles into approximately two
70 ms frames followed by one 20 ms frame; the alternate lock largely alternates
35 ms and 80 ms frames. It has a lower median but a slightly worse mean and a
worse tail. These patterns are observed frame sequences, not a proven account of
worker/lock scheduling. The default's median also agrees with the prior report's
69.85 ms trial, but that agreement does not make individual trials statistical
confidence intervals. The alternate-lock repeat shows the same alternating
pattern and does not improve the mean or slow-frame tail.

Memory means shared WASM linear-memory capacity, not live allocations or whole
browser resident memory. It cannot shrink. Talc's growth is not by itself proof
of a leak; its `WasmGrowAndClaim` source creates separate heaps rather than
extending a previous one, and upstream documents the fragmentation tradeoff.
This is an observation about this configuration, not all Talc configurations.

Every measured trial made zero mesh geometry-buffer writes during orbiting.
The 1240×960 browser screenshots after the timed replay are byte-for-byte equal
after decoding to RGBA, both across all three early-playback variants and
between default and alternate-lock midpoint variants. This checks that the
displayed result is unchanged; it is not an exhaustive allocator correctness test.

Individual sequential trials, not a statistical confidence claim. As in the
previous report, CPU submission does not wait for GPU completion, and progress
notifications are not polled during orbit. Safari was not tested.

## Decision

Keep Rust's default allocator. The early-playback improvement is small; Talc's
tested configuration substantially grows linear memory, and the alternate lock
does not improve the midpoint's mean or slow-frame tail. Neither is a clean
responsiveness win. This does not rule out a genuinely scalable allocator, but
porting or maintaining one is beyond this bounded browser experiment.

The experimental global allocator, feature, and dependencies were removed after
measurement. The normal browser bundle was never replaced. Keep the opt-in orbit
replay and this report; there is no allocator fork or new production dependency.

The next useful investigation is allocation-heavy worker preparation and its
cancellation boundaries. `SoftwareScene::new` currently compiles each scene object
inside each camera-dependent render request, checking cancellation between
objects, not during an object's compilation. The earlier component benchmark
measured roughly 141–171 ms of midpoint scene compilation. That is a concrete
candidate for profiling, not proof that compilation explains all orbit stalls.
Reusing camera-independent preparation should use the existing dependency-tracked
computation graph; no event-specific cache or blanket suppression of work on drag.

## Reproduction notes

The default build and runner commands are in the [orbit report](browser-orbit-profile-2026-09-19.md#reproduce).
The runner also accepts `package=orbit-talc-pkg` or `package=orbit-lock-pkg` to
select a separately generated module. The main instance and all workers load the
same module and share its memory. Generated candidate bundles are ignored and
are local artifacts, not committed dependencies.

To reconstruct the Talc variant, add optional WASM dependencies `talc = "=5.1.1"`
and `spinning_top = "=0.3.0"`, then enable this global allocator only in the
diagnostic build:

```rust
use talc::{sync::TalcLock, wasm::{WasmBinning, WasmGrowAndClaim}};

#[global_allocator]
static ALLOCATOR: TalcLock<
    spinning_top::RawSpinlock,
    WasmGrowAndClaim,
    WasmBinning,
> = TalcLock::new(WasmGrowAndClaim);
```

The lock-only variant uses `dlmalloc = "=0.2.14"` and `spinning_top = "=0.3.0"`.
A `GlobalAlloc` adapter owns `Spinlock<dlmalloc::Dlmalloc>` initialized with
`Dlmalloc::new()`. Each method acquires that one lock and forwards the original
pointer, size, and alignment to `malloc`, `calloc`, `free`, or `realloc`.
No per-thread heaps, TLS, delayed frees, or special cross-thread handoff are added.
Neither prototype is retained as a supported allocator API.

Build each candidate using the same `sandbox-cargo web-threaded` command, adding
only its opt-in feature, then run `wasm-bindgen` into its separate package folder.
The recorded trials are:

```sh
node tools/profile-web-orbit.cjs 'position=0.02&refined&threads=8' alloc-default-early1
node tools/profile-web-orbit.cjs 'position=0.02&refined&threads=8&package=orbit-talc-pkg' talc-early1
node tools/profile-web-orbit.cjs 'position=0.02&refined&threads=8&package=orbit-lock-pkg' lock-early1
node tools/profile-web-orbit.cjs 'position=0.5&refined&threads=8' alloc-default-mid1
node tools/profile-web-orbit.cjs 'position=0.5&refined&threads=8&package=orbit-lock-pkg' lock-mid1
node tools/profile-web-orbit.cjs 'position=0.5&refined&threads=8&package=orbit-lock-pkg' lock-mid2
```

Raw JSON samples, CPU profiles, and screenshots are under ignored `target/orbit-*`.
The runner now also reports the mean to make alternating-frame patterns less
likely to hide behind a favorable median; earlier files retain their raw samples.
