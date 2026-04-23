# prototype-haskell

Two parallel prototypes exploring rendering platforms for the
CAD/CAM substrate. Engine code (model, embedded language, geometry)
will eventually be shared; for now each is a separate cabal project
with its own minimal hello world.

## imgui/

Native ImGui via `dear-imgui` + SDL2 + OpenGL3. Immediate-mode —
redraw everything every frame.

```sh
brew install sdl2
cd imgui
cabal run prototype-haskell-imgui
```

## wasm/

Haskell compiled to WASM via the GHC WASM backend, talking to the
DOM through JSFFI. The HTML host (`index.html`) loads the `.wasm`
output and wires up a click handler.

Requires the WASM-targeted GHC cross-compiler:

```sh
ghcup config add-release-channel cross
ghcup install ghc 9.10 --target wasm32-wasi
cd wasm
wasm32-wasi-cabal build
# Copy/symlink the produced .wasm next to index.html, then
# serve the directory with any static server:
python3 -m http.server 8000
```

## tauri/

Not started yet. Plan: wrap the wasm/ frontend in a Tauri shell
for desktop distribution. Adds Rust + tauri-cli prerequisites; the
WASM bundle is the same.
