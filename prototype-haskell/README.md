# prototype-haskell

Haskell/Wasm prototype for Progred.

The current direction is Haskell compiled to Wasm via GHC's Wasm
backend, talking to the DOM through JSFFI. The HTML host (`index.html`)
loads the `.wasm` output and wires up a click handler.

Requires the WASM-targeted GHC cross-compiler:

```sh
ghcup config add-release-channel cross
ghcup install ghc 9.12 --target wasm32-wasi
```

`cabal.project` selects `wasm32-wasi-ghc`, so `cabal build` and the
Makefile both use the Wasm toolchain by default.

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

Editor note: Haskell language servers installed for native GHC may not
work against this prototype yet. The source depends on the Wasm GHC
toolchain, including JSFFI and `base` from the Wasm compiler.

The native GTK and ImGui probes were removed. They proved basic native
Haskell GUI viability, but the active question is now whether Haskell can
own the model/projection logic while the DOM remains the rendering and
distribution substrate.
