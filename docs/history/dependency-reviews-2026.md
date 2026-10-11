# Dependency reviews, 2026

Dated records moved from [build security](../build-security.md) and the
[release checklist](../release-checklist.md) on 2026-10-11. The current policy
and exceptions are in those documents.

## `chacha20` publish-age exception, 2026-08-28

Adding Fidget 0.5.0 required the `resolver.incompatible-publish-age` exception
for `chacha20` 0.10.2: Fidget's `rand` requirement cannot resolve to the yanked
0.10.0 or 0.10.1 releases. The 0.10.2 crate was checked against the signed
release in the official RustCrypto repository; it has no build script and
contains a focused SIMD feature-detection correction. Unrelated too-new
transitive releases were downgraded before checking in the lockfile.

## Fidget constant-field GPU validation, 2026-09-05

Fidget 0.5.0's GPU `RenderShape` allocates its variable buffer from the compiled
variable count. A constant field therefore creates a zero-byte buffer and tries
to bind it as storage. Creating a zero-sized WebGPU buffer is allowed; binding
a zero-sized range is not. This is platform-independent validation, not a Metal
quirk; see [WebGPU buffer creation](https://gpuweb.github.io/gpuweb/#buffer-creation)
and [bind-group validation](https://gpuweb.github.io/gpuweb/#dom-gpudevice-createbindgroup).
The original Progred crash didn't preserve the GPU validation message, so its
precise cause was inferred from this code path.

Upstream checked on 2026-09-05 at main commit
[`94123ea`](https://github.com/mkeeter/fidget/blob/94123ea4a351fedf66df404c31f3fed0e1474b0e/fidget-wgpu/src/lib.rs#L280-L293):
this was already fixed by allocating at least four bytes. The change landed on
2026-08-25 in [PR #461](https://github.com/mkeeter/fidget/pull/461), commit
[`19a9049`](https://github.com/mkeeter/fidget/commit/19a904926a72a4e8509d02060bfef109fe37e488).
Searches of open and closed issues and PRs for constants, buffers, and bindings
found no separate matching bug report. The older
[PR #318](https://github.com/mkeeter/fidget/pull/318) fixes a different
constant-evaluation problem. The ignored GPU regression test's attempted run
inside Seatbelt found no Metal adapter.

## Fidget Git revision, 2026-09-13

Fidget was pinned to upstream commit
[`0c89e87`](https://github.com/mkeeter/fidget/commit/0c89e87e1b3a6d15cc0976ab6ff05a09f9cf91d6)
(2026-09-12) for its per-object GPU color support, not a local rendering patch.
The revision also includes the constant-field fix and lazy pipeline
construction. The upstream changelog then called the next release 0.5.1
(unpublished).

Review covered the 42-commit delta from 0.5.0, commit provenance, changed
manifests and build scripts, and current advisory and news checks. This was a
reviewed exception to the age preference for the Git revision; registry
resolution kept the seven-day minimum and existing unrelated versions. Cargo's
built-in Git transport hit a TLS error;
`./tools/sandbox-cargo resolve --config net.git-fetch-with-cli=true` used the
system Git transport within the same sandbox.

The RustSec scan had no vulnerability errors, but retained six warnings for the
existing `im`, `sized-chunks`, `bitmaps`, and `ttf-parser` versions, including
unsoundness advisories for the first two.

## Fidget vendoring and JIT, 2026-09-15

Two packages from that revision were vendored with local patches: large-tape
AArch64 JIT fixes and finer-grained raster cancellation (more packages and
patches followed; see [vendor/README.md](../../vendor/README.md)). Registry
additions for `dynasmrt` and its assembler dependencies resolved with the
seven-day minimum; unrelated locked versions were retained. A fresh RustSec
scan found no new advisories. The 196 headless JIT tests were also exercised in
a minimal signed bundle with the app's entitlements; that wasn't a GUI
interaction test.

## Browser Rayon pool, September 2026

The browser Rayon pool added `wasm-bindgen-rayon` 1.3.0 with `no-bundler`. Its
three added registry packages (`wasm-bindgen-rayon`, `wasm_sync`, and
`crossbeam-channel`) resolved under the seven-day publication-age policy;
unrelated locked versions were retained. Browser WebGPU used the existing
Vello/wgpu dependency graph and shaders. The advisory scan after these
additions found no new advisories.

## Fidget 0.5.1 and the wgpu 30 stack, 2026-10-11

Fidget moved from the `0c89e87` Git pin to the published 0.5.1 (2026-09-27).
The published sources of the four vendored packages matched upstream's `v0.5.1`
tag. Only four upstream commits separate the two: a cached color bind group, a
comment fix, a dependency update to wgpu 30, and the release. Seven local
patches re-applied unchanged; the core SSA access patch was dropped as unused,
and the GPU spill experiment was reduced to its default rejection of spilling
tapes. Leaving the Git source dropped the extra packages that upstream's real
`workspace-hack` had pulled into the lockfile.

Sharing a wgpu device with Fidget required the same wgpu major version, so
Vello moved from 0.9 to 0.11 with wgpu 30.0.1, along with parley 0.11.1 and
skrifa 0.44. Vello 0.11's README calls the `vello` crate an experimental
research renderer and names `vello_gpu` as the production one. Resolution used
`tools/linux-cargo update --workspace` with the seven-day minimum publish age.
The native build, the threaded browser build's types, and the tests were
checked on Linux only. The ignored GPU tests, including the compositor pixel
checks and Fidget's constant-field and color test, passed on Mesa's lavapipe
software Vulkan driver. `make perf` before and after the upgrade, on the same
Linux VM, found no change beyond its noise; both runs are in the
[performance log](../performance-log.tsv).

## `im` to `imbl`, 2026-10-11

The cell table moved from `im` 15.1.0, unreleased since April 2022, to `imbl`
7.0.2 (2026-09-09), its maintained fork with the same persistent hash map API;
only the import changed. RustSec lists `im` and `sized-chunks` as unmaintained
and unsound, the latter with a panic-safety use-after-free in the chunk type
`im`'s maps are built on, and `bitmaps` 2 as unmaintained. `imbl` replaces them
with `imbl-sized-chunks` 0.2.0, the release that fixes the same bug, and adds
`archery`, `wide` 0.7 and `safe_arch` 0.7, none with advisories. `rand_core`
and `rand_xoshiro` moved to 0.9.5 and 0.7.0. The remaining warning is
`ttf-parser`'s, reached through winit's Linux window decorations.

Resolution used `tools/linux-cargo update --workspace` with the seven-day
minimum publish age. Advisories were checked against a clone of the RustSec
database on Linux; `sandbox-cargo audit` hasn't run. Interleaved `make perf`
runs on one Linux VM, two pairs of three rounds and four more pairs on the three
workloads that first looked slower, found no change beyond noise; the first
pair is in the [performance log](../performance-log.tsv).
