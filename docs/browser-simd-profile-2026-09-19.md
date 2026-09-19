# Browser SIMD experiment — 2026-09-19

## Decision

Enable `+simd128` in the existing shared-memory browser build. This is a small
compiler-flag improvement, not a new Fidget backend. Native builds, worker
count, cancellation, mouse-up scheduling, render quality, and allocators are
unchanged. No Fidget source patch is part of this experiment.

The browser now needs WebAssembly SIMD support. This uses ordinary fixed-width
SIMD, not relaxed SIMD or fast-math flags. Rust documents explicit feature
selection in its [WASM target notes](https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html#enabled-webassembly-features).
The pinned nightly's default target configuration did not enable `simd128`.

## Measurement

Same headless Command+9 diagnostic as the
[browser/native comparison](browser-native-profile-2026-09-19.md): actual stock
subtraction, 512×512 physical pixels, final-quality 4× depth sampling, eight
rendering workers. Apple M3 Pro, Chrome 153.0.8010.48, pinned nightly-2026-08-27,
release builds. The only build difference was `+simd128`, applied to the entire
WASM build including rebuilt std. Neither build launched the editor.

Ran baseline A, SIMD A, SIMD B, baseline B sequentially, with no concurrent
builds. Each batch had four trials at 2% playback and four at 50%. Discard the
first trial at each position in each batch; pool the remaining six observations
per build/position for the following medians:

| Work | Baseline | SIMD | Time reduction |
| --- | ---: | ---: | ---: |
| Implicit image, 2% | 514 ms | 478 ms | 7.0% |
| Implicit image, 50% | 9.265 s | 8.252 s | 10.9% |
| Stock mesh, 2% | 150 ms | 138 ms | 7.9% |
| Stock mesh, 50% | 2.639 s | 2.412 s | 8.6% |

These are noisy desktop measurements, not a precise universal speedup. Per-batch
implicit medians show the variation rather than hiding it:

| Batch | Implicit, 2% | Implicit, 50% |
| --- | ---: | ---: |
| Baseline A | 506 ms | 8.488 s |
| SIMD A | 470 ms | 8.229 s |
| SIMD B | 502 ms | 8.276 s |
| Baseline B | 537 ms | 9.532 s |

The midpoint gain is about 3% comparing the A batches and 13% comparing the B
batches. SIMD is worth keeping for this small build-only change, but it does
not close the native JIT gap. The generated diagnostic WASM also shrank from
10,137,435 to 10,021,691 bytes; this is not an editor download-size measurement.

Implicit timings include scene preparation, rasterization, shading/depth
assembly, and creation of partial publications. Publications are discarded by
this diagnostic rather than presented. They exclude editor rebuilding, image
upload, browser startup, and display latency. They are not orbit timings or a
measurement of the cached-scene-only path. No Safari timing was collected.

## Output checks and generated code

The diagnostic now fingerprints the complete final RGBA bytes and the
little-endian bytes of every f32 depth value, outside the render timer. All
32 primary trials agree at each playback position across baseline and SIMD on
these fingerprints, coverage, color/depth sums, stock vertex/triangle counts,
and toolpath segment counts.

| Playback | RGBA fingerprint | Depth fingerprint |
| --- | --- | --- |
| 2% | `705e1f9dcd106553` | `6f0dc9e04155606f` |
| 50% | `12b1e29a303a80a0` | `d4ff62c840076406` |
| 100%, 256×256 | `4a659f9e1612be2d` | `d26b58abd005df53` |

Fingerprints use FNV-1a over the full buffers; this is a regression check, not a
claim that every possible scene has been proven equivalent. A separate
completed-playback comparison at 256×256 (two trials per build) also matched.

The normal editor bundle was rebuilt with SIMD. The existing headless WebGPU
and forced Canvas2D checks both passed (ordering, clipping, partial depth,
image upload, resizing, geometry replacement), as did all six website-server
tests. Interactive validation remains with the user.

LLVM disassembly confirms actual vector arithmetic in the SIMD artifact,
including `f32x4.mul`, `f32x4.add`, and `f32x4.div`. However, the inspected
`VmFloatSliceEval::eval` bodies still use scalar arithmetic. Enabling SIMD does
not automatically turn the whole Fidget interpreter into a four-wide renderer.
Changing those loops would be a separate experiment, not part of this flag change.

## Reproduce

Use the existing sandboxed `cam_profile` build and browser runner. The runner's
optional final argument selects a generated diagnostic package, so comparisons
do not overwrite the normal editor bundle:

```sh
./tools/sandbox-cargo web-threaded build --release -p progred --features cam-profile --example cam_profile
wasm-bindgen --target web --no-typescript --out-dir web/profile-simd-pkg --out-name profile \
  target/sandbox/build-web/wasm32-unknown-unknown/release/examples/cam_profile.wasm
node tools/profile-web-cam.cjs 512 4 0.02,0.5 none 8 profile-simd-pkg
```

For the scalar control, remove only `,+simd128` from the browser flags in
`tools/sandbox-cargo`, rebuild through that wrapper, and generate `web/profile-pkg`.
Restore the flag before building the editor. Do not bypass the Cargo sandbox or
override its wrapper. The experiment used a temporary separate SIMD target
directory; that temporary command was removed after deciding to enable SIMD.

Ignored raw artifacts:
`target/cam-simd-{baseline,candidate}-{a,b}.jsonl` and the corresponding
`*-final.jsonl` completed-playback checks. These are local diagnostics, not
production dependencies.
