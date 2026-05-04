PORT ?= 8000
WASM_GHC ?= wasm32-wasi-ghc
WASM_GHC_PKG ?= wasm32-wasi-ghc-pkg
WASM = prototype-haskell-wasm.wasm

.PHONY: build link run serve

build:
	cabal build -w $(WASM_GHC) --with-hc-pkg=$(WASM_GHC_PKG)

link: build
	@wasm=$$(find dist-newstyle/build/wasm32-wasi -type f -name "$(WASM)" | head -n 1); \
	if [ -z "$$wasm" ]; then echo "Could not find built wasm output"; exit 1; fi; \
	ln -sf "$$wasm" "$(WASM)"

run: link serve

serve:
	python3 -m http.server $(PORT)
