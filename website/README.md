# prog.red website

This is the local website foundation. It embeds the actual browser editor;
there is no separate demo implementation, framework, or hosting dependency.

## Open it

On macOS, double-click **Preview.command** in Finder. It builds the browser
editor using the repository's isolated Cargo workflow, starts a loopback-only
web server, and opens the website in your default browser. Leave its Terminal
window running; Control+C stops the server. The first build can take a while.

The browser build uses `nightly-2026-08-27` with `rust-src` and `llvm-tools`,
and `wasm-bindgen-cli` matching the locked `wasm-bindgen` version. It rebuilds
`std` with atomics for shared-memory workers, inside the normal Cargo sandbox.
One-time setup is described in [the browser host notes](../web/README.md).
No Node or package installation is needed for the site.

Opening `public/index.html` directly is not supported: the WebAssembly module
and JavaScript imports need HTTP rather than a `file://` origin.

After the first build, `python3 website/preview.py` from the repository root
starts the preview without rebuilding the editor. `--no-open` leaves the browser
closed, and `--port 8081` requests a particular port. By default the operating
system chooses an available port, so an existing editor preview can stay open.

## Work on it

- `public/index.html` and `public/style.css` are the website. Edit and refresh.
- `/editor/` serves the existing `web/` host and generated `web/pkg/` build.
- Editor changes require `make build-web` and a refresh.
- The server exposes only these two directories, not the repository or its Git data.
- The iframe owns an independent editor session. Full-page editor links open a
  new session; they do not transfer the embedded document.
- Browser edits are currently in memory only. Do not author something you need
  to keep here yet; document import/export is a useful next step.

The initial page leaves the editor blank, with its existing Examples menu
available. We can build small examples and their explanations from here without
settling the whole site structure first. Nothing is deployed, and `prog.red`'s
DNS is unchanged.

CAM stock meshing and implicit rendering use a shared-memory coordinator and
Rayon worker pool (up to eight rendering workers). The editor starts only after
the pool is ready; there is no synchronous computation fallback.
The server supplies the COOP/COEP headers required for cross-origin isolation
on both the website and embedded editor. Existing preview servers need to be
restarted after this change, not just refreshed. Projection and toolpath generation
remain on the page thread. Presentation uses the native compositor via WebGPU,
including GPU mesh drawing. Canvas2D/CPU mesh drawing remains a fallback when
WebGPU is unavailable.

For deployment, use HTTPS and configure those same headers. GitHub Pages alone
does not provide the header configuration this threaded build requires; no
service-worker workaround or deployment is configured here.

Run the preview server checks without opening an editor:

```sh
python3 -B -m unittest discover -s website -p 'test_*.py'
```
