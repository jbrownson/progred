# prototype-haskell

Haskell/Wasm prototype for Progred.

Status: this is a Haskell/Wasm/Tauri spike for testing whether Haskell
can own UI state and event interpretation while a thin JavaScript host
only forwards browser events and provides drawing primitives. The current
demo renders directly to a `<canvas>` through JSFFI. Haskell owns the
model, focus state, hit testing, keyboard handling, and draw-command
generation.

This prototype's direction is Haskell compiled to Wasm via GHC's Wasm
backend, talking to Canvas/Web APIs through JSFFI. The HTML host
(`index.html`) loads the `.wasm` output, resizes the canvas, forwards
pointer/key events, and exposes a small `window.progredCanvas` drawing
surface.

Requires a native GHC for editor/typechecking and the WASM-targeted GHC
cross-compiler for the app bundle:

```sh
ghcup install ghc 9.12.2
ghcup config add-release-channel cross
source ~/.ghc-wasm/env
ghcup install ghc wasm32-wasi-9.12.2.20250327 -- --host=aarch64-apple-darwin --target=wasm32-wasi --with-intree-gmp --with-system-libffi
```

The default `cabal.project` selects native `ghc-9.12.2` so editor
tooling can typecheck the Haskell code. `cabal-wasm.project` selects
`wasm32-wasi-ghc` for the real app bundle, and the Makefile uses that
project file when building the Wasm executable. The Makefile invokes the
cross compiler through `ghcup run` instead of changing the global active
GHC, so native editor tooling can keep using the native compiler.
It still expects the ghc-wasm-meta environment at `~/.ghc-wasm/env` for
the WASI SDK, Node, Binaryen, and related tools.

Run in a browser:

```sh
make run
```

Run in Tauri:

```sh
cargo install tauri-cli --locked
make tauri-dev
```

Build a Tauri app bundle:

```sh
make tauri-build
```

The Makefile builds the Haskell/Wasm executable, generates
`ghc_wasm_jsffi.js`, copies the runtime files into `dist/`, and then
serves or wraps that static directory.

`ghc_wasm_jsffi.js` is the generated JSFFI import object for the current
Haskell source. It is generated as part of `make dist`.

Editor note: the only target-specific code lives under `platform/`.
`Progred.Platform` is selected by Cabal from either `platform/stub/` or
`platform/wasm/`. Native HLS sees undefined stubs; the real app bundle is
still built with the Wasm GHC toolchain and the JSFFI implementation.
Wasm-only JS exports live in `platform/wasm/Progred/Wasm/Exports.hs`.

The native GTK and ImGui probes were removed. They proved basic native
Haskell GUI viability, but the active question is now whether Haskell can
own the UI model/projection/event logic while the browser/webview remains
the rendering and distribution substrate.

## Layout Notes

Layout is deliberately tiny and explicit in `src/Main.hs` for now. The
next useful step may be working on Progred itself and revisiting layout
only when the app needs more help.

If this prototype does grow a layout layer, keep three concerns separate:

1. Box layout: Nic Barker's C Clay layout library is still a good source
   of inspiration for a small, fast, flex-like row/column model with
   fixed/grow/fit sizing, padding, gaps, and measured leaves. There does
   not appear to be a mature Haskell binding for that Clay. The Haskell
   package named `clay` is a CSS EDSL, not that layout engine.

2. Text flow: Clay includes text wrapping, but its core value here is box
   layout rather than text layout. For real text flow, look separately at
   projects such as Pretext, Parley, or Cosmic Text. Progred is unlikely
   to need giant blocks of prose early, so do not make this a dependency
   until a real use case appears.

3. Structured pretty layout: Progred will need graph/projection layouts
   that decide whether a structure fits on one line or should become
   multiline. This is closer to Wadler/Leijen pretty-printing than to
   browser text flow. Start from the pretty-printer model before
   inventing ad hoc single-line/multiline policy.
