# prototype-haskell

Haskell/Wasm prototype for Progred.

The current direction is Haskell compiled to Wasm via GHC's Wasm
backend, talking to the DOM through JSFFI. The HTML host (`index.html`)
loads the `.wasm` output and wires up a click handler.

Requires the WASM-targeted GHC cross-compiler:

```sh
ghcup config add-release-channel cross
ghcup install ghc 9.12 --target wasm32-wasi
cabal build -w wasm32-wasi-ghc --with-hc-pkg=wasm32-wasi-ghc-pkg
```

The checked-in `index.html` expects `prototype-haskell-wasm.wasm` next
to it. Copy or symlink the built wasm output into this directory, then
serve the directory with any static server:

```sh
python3 -m http.server 8000
```

`ghc_wasm_jsffi.js` is the generated JSFFI import object for the current
Haskell source. If the JSFFI declarations change, regenerate it with the
GHC Wasm post-linker.

The native GTK and ImGui probes were removed. They proved basic native
Haskell GUI viability, but the active question is now whether Haskell can
own the model/projection logic while the DOM remains the rendering and
distribution substrate.
