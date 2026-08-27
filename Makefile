UNAME_S := $(shell uname -s)

ifeq ($(UNAME_S),Darwin)
NATIVE_RUN_TARGET := run-macos
else ifeq ($(UNAME_S),Linux)
NATIVE_RUN_TARGET := run-linux
else
NATIVE_RUN_TARGET := unsupported-native-platform
endif

.PHONY: help run run-native run-macos run-linux unsupported-native-platform dev dev-native build-web run-web web serve-web sandbox-fetch sandbox-check sandbox-build sandbox-test sandbox-app sandbox-web

help:
	@echo "Development:"
	@echo "  make run          Run the native app once (alias: run-native)"
	@echo "  make dev          Relaunch the native app after Ctrl+C (alias: dev-native)"
	@echo "  make build-web    Build the browser app"
	@echo "  make run-web      Build and serve the browser app on port 8080"
	@echo
	@echo "Native dev controls: Ctrl+C restarts; Ctrl+\\ quits"

run: run-native

run-native: $(NATIVE_RUN_TARGET)

run-macos: sandbox-app
	@/usr/bin/open -W -n target/sandbox/app/Progred.app

run-linux:
	@./tools/run-linux $(ARGS)

unsupported-native-platform:
	@echo "native Progred is not supported on $(UNAME_S)" >&2
	@exit 1

dev: dev-native

dev-native:
	@trap ':' INT; \
	trap 'exit 0' QUIT TERM HUP; \
	trap 'pkill -x progred >/dev/null 2>&1 || true' EXIT; \
	while true; do \
		pkill -x progred >/dev/null 2>&1 || true; \
		echo "Ctrl+C restarts Progred; Ctrl+\\ quits"; \
		$(MAKE) run-native || true; \
	done

build-web: sandbox-web

run-web: build-web
	python3 -m http.server 8080 --bind 0.0.0.0 --directory web

# Compatibility aliases.
web: build-web

serve-web: run-web

sandbox-fetch:
	./tools/sandbox-cargo fetch

sandbox-check:
	./tools/sandbox-cargo check --workspace

sandbox-build:
	./tools/sandbox-cargo build --workspace

sandbox-test:
	./tools/sandbox-cargo test --workspace

sandbox-app:
	@./tools/build-macos-app

sandbox-web:
	./tools/sandbox-cargo build --release -p progred --target wasm32-unknown-unknown
	wasm-bindgen --target web --no-typescript --out-dir web/pkg --out-name progred target/sandbox/build/wasm32-unknown-unknown/release/progred.wasm
