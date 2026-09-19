# Shared-memory browser worker experiment — 2026-09-19

## Status

**Integrated after the synchronization fixes below.** `make build-web` and
`website/Preview.command` now build the shared-memory editor, initialize its
worker, and only then start the UI. This keeps the ordinary Rust `Send` job
interface; no scene-specific serialization or browser-only computation API was
introduced. The historical failed probe remains documented below.

The browser is primarily a demo host. This was a bounded attempt to keep the
native job interface intact, not approval to redesign the core around browsers.

## What ran

The `web_worker_probe` Rust example uses the real `incremental::Runtime`,
`Tasks`, cancellation token, and latest-job replacement scheduler. It does not
start Progred or draw a frame. The JS host instantiates the same WASM module
in the page and one worker, sharing memory. A double-boxed `Job` becomes a thin
pointer passed through `postMessage`; the worker consumes that pointer exactly
once. Captures and results remain Rust objects; they are not serialized into JS.

Build requirements are the pinned nightly, atomics-enabled rebuilt `std`,
shared/imported memory, the TLS exports used by wasm-bindgen, and cross-origin
isolation headers. The normal browser build now uses those same requirements.

## Results before the shared-flag change

Headless Google Chrome, macOS, release build:

| Probe | Observed result |
| --- | --- |
| Worker computes using a captured Rust value; page timer cancels its loop; `Tasks` publishes 42 | Pass |
| 50,000 input replacements, worker registers a no-op cancellation callback | UI-thread trap, reproduced in two runs |
| 50,000 input replacements without callback registration | Pass in one run; not evidence that all other locks are safe |

The failure is:

```text
RuntimeError: Atomics.wait cannot be called in this context
std::sys::sync::mutex::futex::Mutex::lock_contended
incremental::Cancellation::cancel
incremental::background::AsyncNode::refresh
```

The callback itself did no work. Registering it on the worker and cancelling
on the UI thread contended on the token's callback-list mutex. Fidget mesh and
raster jobs used `on_cancel` to bridge cancellation to Fidget.
The stress test accelerates that overlap; it does not measure the frequency of
failures during human slider dragging.

This was not a Rust data race or a discovered native cancellation bug. The native
lock is legitimate, but Rust's contended WASM mutex can use `Atomics.wait`, which
[cannot block the browser main thread](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Atomics/wait).
The scheduler's replacement slot also shared a mutex across those threads;
even fixing cancellation alone would not establish safety. A WASM trap is not Rust error unwinding,
so the diagnostic ends and discards that page after a failure.

## Shared-flag change

After approval, `Cancellation` was simplified to `Arc<AtomicBool>` and the
callback registry removed. Both native and browser meshing/rasterization now
construct Fidget tokens from that same flag, through the small independent
[Fidget API patch](experiments/fidget-core-cancellation.patch). The libraries
remain independent: the shared type belongs to `std`. Tokens created after
cancellation observe it immediately. A new request gets a new flag, never a
reset of an old flag.

The current browser probe uses Fidget's actual token instead of registering a
callback. The page timer successfully cancels the worker's Fidget token, and
three separate 50,000-replacement trials all completed without a trap. These
results verified that cancellation fix, not all possible scheduler races. That
initial pass left the pending-job slot and completion channel unchanged.

## Scheduler and browser integration

The next pass replaced the shared pending-slot mutex with an atomic scheduled
flag plus `concurrent-queue`'s single-element replaceable queue. Reports use its
unbounded queue instead of a blocking-capable channel implementation. This is
an existing workspace dependency; no bespoke unsafe queue was added. Ownership
handoff releases the scheduled flag before checking for replacements, so a
concurrent submission either schedules itself or is picked up by the finishing
worker. The same node still has at most one running computation and one newest
replacement; native queue/cancellation/revision semantics are unchanged. These
queues may briefly retry atomic operations, but do not wait on an OS mutex or
wait for user computation to finish.

`progred::web_worker` and `web/worker-host.js` are now the shared transport for
both the editor and diagnostics. JS forwards job pointers and wake messages;
the main thread delivers wakes to Winit, coalesced to animation frames. It
never sends Winit's proxy itself across the worker boundary. The normal WASM
entrypoint no longer starts the editor implicitly when the worker instantiates
the module. Initialization/trap errors are visible, with no inline fallback.

Progress and partial image publication now use `web-time` rather than disabling
mid-pass updates on WASM. Native timing is still `std::time::Instant` through
that portable API. These clocks and assembly locks stay worker-local. The
first integration used one worker, not a Rayon Web Worker pool. A subsequent
pass adds the pool described below; Fidget remains VM-based on WASM.

Headless Chrome results after integration (not editor interaction tests):

- Shared closure, page-timer cancellation through Fidget's token, completion: pass.
- Three runs of 50,000 latest-request replacements: pass.
- Fidget block with 36 spherical cuts: 34,711 mesh vertices, 27,889 occupied
  raster pixels; 33 page timer ticks during the diagnostic, maximum gap 11 ms.
- Replacing that request with 12 cuts during computation: current result only,
  14,483 mesh vertices; 25 timer ticks, maximum gap 11 ms.

The Fidget diagnostic uses the real VM mesher/rasterizer and production job
transport, but does not run the editor or exercise its visual interactions.

## Reproduce without launching the editor

Install the pinned toolchain's `rust-src` and `llvm-tools` components using
rustup if absent. `llvm-tools` supplies the LLVM library needed by this macOS
toolchain's `rust-lld`:

```sh
rustup component add rust-src llvm-tools --toolchain nightly-2026-08-27
./tools/sandbox-cargo web-threaded-fetch
./tools/sandbox-cargo web-threaded build --release -p progred --example web_worker_probe
wasm-bindgen --target web --no-typescript --out-dir web/probe-pkg --out-name probe \
  target/sandbox/build-web/wasm32-unknown-unknown/release/examples/web_worker_probe.wasm
node tools/test-web-worker.cjs
```

The last command requires Playwright to be available to Node and an installed
Google Chrome. It starts the local website server with `--no-open`, opens only
`/editor/worker-probe.html`, prints results, and
closes its own browser/server. It intentionally exits nonzero on a reproduced
failure. Passing stress trials do not establish that all synchronization is
safe on the browser main thread. Generated probe
files are ignored and separate from `web/pkg`, so the website's editor build is
not overwritten. The server currently also requires the ordinary browser editor
to have been built.

## Remaining boundaries

### Parallel rendering and browser GPU presentation

The coordinator now awaits `wasm-bindgen-rayon`'s bundlerless pool before
announcing readiness. Ordinary Fidget parallel meshing and rasterization use
that pool. The default is `max(1, min(8, hardwareConcurrency - 1))`; diagnostics
can supply a different positive count. Rayon waits happen on the coordinator,
not on the UI thread. Native scheduling and evaluator interfaces are unchanged.

Rayon owns its nested workers, so their progress callbacks cannot use the
coordinator's direct `postMessage` route. A per-instance random channel name
is installed in shared Rust state before starting workers. `worker-host.js`
opens that BroadcastChannel lazily in each thread; only the page listens and
coalesces wakes to animation frames. Reports themselves remain in the ordinary
Rust scheduler queues. The channel also forwards any nested job submission.
Startup timeouts and uncaught worker failures are fatal, rather than falling
back to main-thread computation.

Browser presentation now reuses the native Vello compositor and triangle
renderer over WebGPU, preserving ordered vectors/images/meshes, clips, and
partial implicit depth replacement. Setup is asynchronous before Winit starts;
the page retains the renderer independently of editor/frame state. No GPU
readback is used in normal presentation. When no WebGPU adapter exists, the
Canvas2D backend remains available. Its image upload now explicitly copies to
a JS-owned typed array: ImageData rejects SharedArrayBuffer-backed arrays.
This is a platform API requirement, not an extra copy on the WebGPU route.

The standalone `web_gpu_probe` example / `tools/test-web-gpu.cjs` checks the
actual browser compositor and fallback without starting an editor. It covers
two meshes with different clips, vectors below/above, partial depth images,
CPU image upload, resizing, and replaced geometry. See the
[before/after measurements](browser-native-profile-2026-09-19.md).

Final headless checks with the pool enabled pass: production module startup,
shared cancellation, three runs of 50,000 request replacements, live progress
before completion, and cancellation/replacement of real parallel Fidget work.
The presentation checks pass for both WebGPU and forced Canvas2D fallback.
The six local server tests, 30 incremental runtime tests, and 780 native editor
tests pass too (45 opt-in editor tests remain ignored). These are not visual
editor interaction tests.

### Still synchronous

This does not make all editor work asynchronous: projection, Grap program
generation, and drawing/compositing the available mesh still run on the page.
Only existing background memo jobs move to the worker. Compiler/geometry stages
without internal cancellation checks may delay a replacement, but no longer
occupy the browser event loop. Worker fatal errors require reloading the page;
arbitrary JS handles must not be hidden inside `Send` jobs. Publishing requires
HTTPS and COOP/COEP configuration, not merely copying files to GitHub Pages.
