# Progred in a browser

From the repository root:

```sh
make serve-web
```

Open `http://127.0.0.1:8080/editor/` on this machine.
The built-in documents are under the **Examples** menu.

The browser build uses a shared-memory coordinator for the existing CAM background
jobs. It requires a secure context (localhost or HTTPS) and cross-origin
isolation. The local server supplies `Cross-Origin-Opener-Policy: same-origin`
and `Cross-Origin-Embedder-Policy: require-corp` for the site, editor, and worker.
A plain file URL, ordinary `python -m http.server`, or LAN HTTP is not sufficient.
Unsupported hosts show an error instead of silently doing heavy work inline.

One-time build prerequisites:

```sh
rustup toolchain install nightly-2026-08-27 --profile minimal
rustup component add rust-src llvm-tools --toolchain nightly-2026-08-27
./tools/sandbox-cargo web-threaded-fetch
```

Also install `wasm-bindgen-cli` matching the lockfile. `make build-web` rebuilds
the standard library with atomics under Seatbelt and generates `web/pkg`.
Native builds continue using stable Rust. The worker instantiates exactly the
same module with shared memory; ordinary Rust `Send` closures and results stay
in Rust, while JS transfers job pointers and wake notifications. The page alone
starts the editor and dispatches completion events. Fatal worker errors require
a page reload; they are not treated as completed computations.

The coordinator initializes a `wasm-bindgen-rayon` pool before accepting jobs.
It uses up to eight rendering workers, leaving one reported hardware thread
free when possible. Fidget uses its existing parallel VM mesher/rasterizer;
this is not Fidget's experimental GPU interpreter or a browser JIT. A per-page
BroadcastChannel carries progress/completion wakes from any pool thread to the
page; the Rust results remain in shared memory.

Presentation uses the same Vello/image/triangle compositor as native, through
WebGPU. Tool/path meshes stay GPU resources across camera changes; implicit
images upload when replaced, with the same depth and clipping behavior. If no
WebGPU adapter is available, the host retains the slower Canvas2D/CPU-mesh
fallback. Startup logs the selected presentation backend and worker count.

`website/Preview.command` builds and opens the whole website. See
[the website README](../website/README.md) and
[worker diagnostics](../docs/web-worker-experiment-2026-09-19.md).
See also the [performance comparison](../docs/browser-native-profile-2026-09-19.md).
