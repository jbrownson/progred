PORT ?= 8000
WASM_GHC_VERSION ?= wasm32-wasi-9.12.2.20250327
GHC_WASM_ENV ?= $(HOME)/.ghc-wasm/env
WASM_TOOL = . "$(GHC_WASM_ENV)" && ghcup run --ghc "$(WASM_GHC_VERSION)" --
WASM = prototype-haskell.wasm
DIST = progred/dist
WEB = progred/web
WASI_SHIM_DIR = $(WEB)/node_modules/@bjorn3/browser_wasi_shim/dist
WASI_SHIM = $(WASI_SHIM_DIR)/index.js

.PHONY: build dist run serve tauri-dev tauri-build

build:
	$(WASM_TOOL) cabal build --project-file=cabal-wasm.project exe:prototype-haskell

$(WASI_SHIM): $(WEB)/package.json $(WEB)/package-lock.json
	npm --prefix $(WEB) ci

dist: build $(WASI_SHIM)
	mkdir -p $(DIST)/vendor/browser_wasi_shim
	@wasm=$$(find dist-newstyle/build/wasm32-wasi -type f -name "$(WASM)" | head -n 1); \
	if [ -z "$$wasm" ]; then echo "Could not find built wasm output"; exit 1; fi; \
	post_link=$$($(WASM_TOOL) wasm32-wasi-ghc --print-libdir); \
	$(WASM_TOOL) node "$$post_link/post-link.mjs" -i "$$wasm" -o "$(DIST)/ghc_wasm_jsffi.js"; \
	cp "$$wasm" "$(DIST)/$(WASM)"
	cp $(WEB)/index.html $(DIST)/
	cp $(WASI_SHIM_DIR)/*.js $(DIST)/vendor/browser_wasi_shim/

run: dist serve

serve:
	python3 -m http.server $(PORT) --directory $(DIST)

tauri-dev: dist
	cd progred/tauri && cargo tauri dev

tauri-build: dist
	cd progred/tauri && cargo tauri build
