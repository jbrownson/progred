PORT ?= 8000
WASM_GHC ?= wasm32-wasi-ghc
WASM = prototype-haskell-wasm.wasm
DIST = dist
POST_LINK = $(shell $(WASM_GHC) --print-libdir)/post-link.mjs

.PHONY: build dist run serve tauri-dev tauri-build

build:
	cabal build

dist: build
	mkdir -p $(DIST)
	@wasm=$$(find dist-newstyle/build/wasm32-wasi -type f -name "$(WASM)" | head -n 1); \
	if [ -z "$$wasm" ]; then echo "Could not find built wasm output"; exit 1; fi; \
	node "$(POST_LINK)" -i "$$wasm" -o "$(DIST)/ghc_wasm_jsffi.js"; \
	cp "$$wasm" "$(DIST)/$(WASM)"
	cp index.html $(DIST)/

run: dist serve

serve:
	python3 -m http.server $(PORT) --directory $(DIST)

tauri-dev: dist
	cd src-tauri && cargo tauri dev

tauri-build: dist
	cd src-tauri && cargo tauri build
