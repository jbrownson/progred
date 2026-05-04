PORT ?= 8000
WASM_GHC ?= wasm32-wasi-ghc
WASM_GHC_PKG ?= wasm32-wasi-ghc-pkg
WASM = prototype-haskell-wasm.wasm
DIST = dist
POST_LINK = $(shell $(WASM_GHC) --print-libdir)/post-link.mjs

.PHONY: build link dist run serve tauri-dev tauri-build

build:
	cabal build -w $(WASM_GHC) --with-hc-pkg=$(WASM_GHC_PKG)

link: build
	@wasm=$$(find dist-newstyle/build/wasm32-wasi -type f -name "$(WASM)" | head -n 1); \
	if [ -z "$$wasm" ]; then echo "Could not find built wasm output"; exit 1; fi; \
	ln -sf "$$wasm" "$(WASM)"

dist: link
	mkdir -p $(DIST)
	node "$(POST_LINK)" -i "$(WASM)" -o "$(DIST)/ghc_wasm_jsffi.js"
	cp index.html $(WASM) $(DIST)/

run: dist serve

serve:
	python3 -m http.server $(PORT) --directory $(DIST)

tauri-dev: dist
	cd src-tauri && cargo tauri dev

tauri-build: dist
	cd src-tauri && cargo tauri build
