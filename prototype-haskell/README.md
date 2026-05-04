# prototype-haskell

Haskell/Wasm prototype for Progred.

The current direction is Haskell compiled to Wasm via GHC's Wasm
backend, talking to the DOM through JSFFI. The HTML host (`index.html`)
loads the `.wasm` output and wires up a click handler.

Requires a native GHC for editor/typechecking and the WASM-targeted GHC
cross-compiler for the app bundle:

```sh
ghcup install ghc 9.12.2
ghcup config add-release-channel cross
ghcup install ghc 9.12 --target wasm32-wasi
```

The default `cabal.project` selects native `ghc-9.12.2` so editor
tooling can typecheck the Haskell code. `cabal-wasm.project` selects
`wasm32-wasi-ghc` for the real app bundle, and the Makefile uses that
project file when building the Wasm executable.

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

Editor note: `Progred.Platform` is selected by Cabal from either
`platform/stub/` or `platform/wasm/`. Native HLS sees undefined stubs;
the real app bundle is still built with the Wasm GHC toolchain and the
JSFFI implementation. Wasm-only JS exports live in
`platform/wasm/Progred/Wasm/Exports.hs`.

The native GTK and ImGui probes were removed. They proved basic native
Haskell GUI viability, but the active question is now whether Haskell can
own the model/projection logic while the DOM remains the rendering and
distribution substrate.
