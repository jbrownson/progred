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

The Makefile builds the Haskell/Wasm executable, copies `index.html`,
`ghc_wasm_jsffi.js`, and `prototype-haskell-wasm.wasm` into `dist/`, and
then serves or wraps that static directory.

`ghc_wasm_jsffi.js` is the generated JSFFI import object for the current
Haskell source. If the JSFFI declarations change, regenerate it with the
GHC Wasm post-linker before running the app.

The native GTK and ImGui probes were removed. They proved basic native
Haskell GUI viability, but the active question is now whether Haskell can
own the model/projection logic while the DOM remains the rendering and
distribution substrate.
