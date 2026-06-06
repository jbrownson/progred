PORT ?= 8000
WASM_GHC_VERSION ?= wasm32-wasi-9.12.2.20250327
GHC_WASM_ENV ?= $(HOME)/.ghc-wasm/env
WASM_TOOL = . "$(GHC_WASM_ENV)" && ghcup run --ghc "$(WASM_GHC_VERSION)" --
WASM = prototype-haskell.wasm
DIST = progred/dist

.PHONY: build dist run serve tauri-dev tauri-build

build:
	$(WASM_TOOL) cabal build --project-file=cabal-wasm.project exe:prototype-haskell

dist: build
	mkdir -p $(DIST)
	@wasm=$$(find dist-newstyle/build/wasm32-wasi -type f -name "$(WASM)" | head -n 1); \
	if [ -z "$$wasm" ]; then echo "Could not find built wasm output"; exit 1; fi; \
	post_link=$$($(WASM_TOOL) wasm32-wasi-ghc --print-libdir); \
	$(WASM_TOOL) node "$$post_link/post-link.mjs" -i "$$wasm" -o "$(DIST)/ghc_wasm_jsffi.js"; \
	cp "$$wasm" "$(DIST)/$(WASM)"
	cp progred/web/index.html $(DIST)/

run: dist serve

serve:
	python3 -m http.server $(PORT) --directory $(DIST)

tauri-dev: dist
	cd progred/tauri && cargo tauri dev

tauri-build: dist
	cd progred/tauri && cargo tauri build
