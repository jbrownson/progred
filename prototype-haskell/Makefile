PORT ?= 8000
WASM_GHC_VERSION ?= wasm32-wasi-9.12.2.20250327
WASM_GHC_SHORT_VERSION ?= $(patsubst wasm32-wasi-%,%,$(WASM_GHC_VERSION))
GHC_WASM_ENV ?= $(HOME)/.ghc-wasm/env
WASM_TOOL = . "$(GHC_WASM_ENV)" && ghcup run --ghc "$(WASM_GHC_VERSION)" --
WASM_GHC_ROOT ?= $(HOME)/.ghcup/ghc/$(WASM_GHC_VERSION)
WASM_GHC ?= $(WASM_GHC_ROOT)/bin/wasm32-wasi-ghc-$(WASM_GHC_SHORT_VERSION)
WASM_HC_PKG ?= $(WASM_GHC_ROOT)/bin/wasm32-wasi-ghc-pkg-$(WASM_GHC_SHORT_VERSION)
WASM_POST_LINK ?= $(WASM_GHC_ROOT)/lib/wasm32-wasi-ghc-$(WASM_GHC_SHORT_VERSION)/lib/post-link.mjs
WASM = prototype-haskell.wasm
DIST = progred/dist
WEB = progred/web
WASI_SHIM_DIR = $(WEB)/node_modules/@bjorn3/browser_wasi_shim/dist
WASI_SHIM = $(WASI_SHIM_DIR)/index.js
NODE ?= node

SANDBOX_XDG_CACHE_HOME ?= /private/tmp/progred-codex-xdg
SANDBOX_CABAL_CONFIG ?= /private/tmp/progred-codex-cabal-config
SANDBOX_CABAL_LOGS ?= $(SANDBOX_XDG_CACHE_HOME)/cabal/logs
SANDBOX_TEST_LOG ?= /private/tmp/progred-codex-progred-tests.log
SANDBOX_REMOTE_REPO_CACHE ?= $(HOME)/.cache/cabal/packages
SANDBOX_STORE_DIR ?= $(HOME)/.local/state/cabal/store
SANDBOX_CABAL = env XDG_CACHE_HOME="$(SANDBOX_XDG_CACHE_HOME)" cabal --config-file="$(SANDBOX_CABAL_CONFIG)"

.PHONY: build dist run serve sandbox-build sandbox-cabal-config sandbox-dist sandbox-test sandbox-wasm-build tauri-dev tauri-build

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

# Cabal's user config may write build-summary logs under ~/.cache, which the
# managed sandbox cannot write. These targets use a temp Cabal config that keeps
# all Cabal logs in /private/tmp while reusing the user's package cache/store.
sandbox-cabal-config:
	mkdir -p "$(SANDBOX_CABAL_LOGS)"
	cabal --config-file="$(SANDBOX_CABAL_CONFIG)" user-config init -f \
	  -a "logs-dir: $(SANDBOX_CABAL_LOGS)" \
	  -a "build-summary: $(SANDBOX_CABAL_LOGS)/build.log" \
	  -a "build-log: $(SANDBOX_CABAL_LOGS)/build.log" \
	  -a "remote-repo-cache: $(SANDBOX_REMOTE_REPO_CACHE)" \
	  -a "store-dir: $(SANDBOX_STORE_DIR)"

sandbox-build: sandbox-cabal-config
	$(SANDBOX_CABAL) build exe:prototype-haskell

sandbox-test: sandbox-cabal-config
	$(SANDBOX_CABAL) test progred-tests --test-log="$(SANDBOX_TEST_LOG)"

# Avoid ghcup run in the sandbox because it writes under ~/.ghcup/logs. Source
# the wasm toolchain env, then pass the concrete compiler paths to Cabal.
sandbox-wasm-build: sandbox-cabal-config
	. "$(GHC_WASM_ENV)" && $(SANDBOX_CABAL) build --project-file=cabal-wasm.project \
	  -w "$(WASM_GHC)" \
	  --with-hc-pkg="$(WASM_HC_PKG)" \
	  exe:prototype-haskell

sandbox-dist: sandbox-wasm-build $(WASI_SHIM)
	mkdir -p $(DIST)/vendor/browser_wasi_shim
	@wasm=$$(find dist-newstyle/build/wasm32-wasi -type f -name "$(WASM)" | head -n 1); \
	if [ -z "$$wasm" ]; then echo "Could not find built wasm output"; exit 1; fi; \
	"$(NODE)" "$(WASM_POST_LINK)" -i "$$wasm" -o "$(DIST)/ghc_wasm_jsffi.js"; \
	cp "$$wasm" "$(DIST)/$(WASM)"
	cp $(WEB)/index.html $(DIST)/
	cp $(WASI_SHIM_DIR)/*.js $(DIST)/vendor/browser_wasi_shim/
