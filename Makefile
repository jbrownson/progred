UNAME_S := $(shell uname -s)

ifeq ($(UNAME_S),Darwin)
NATIVE_RUN_TARGET := run-macos
else ifeq ($(UNAME_S),Linux)
NATIVE_RUN_TARGET := run-linux
else
NATIVE_RUN_TARGET := unsupported-native-platform
endif

.PHONY: help run run-native run-macos run-linux install-linux unsupported-native-platform dev dev-native build-web run-web web serve-web build-ipad build-ipad-device sandbox-fetch sandbox-check sandbox-build sandbox-test sandbox-app sandbox-web

help:
	@echo "Development:"
	@echo "  make run          Run the native app once (alias: run-native)"
	@echo "  make dev          Rebuild after app exit or Ctrl+C (alias: dev-native)"
	@echo "  make build-web    Build the browser app"
	@echo "  make run-web      Build and serve the browser app on port 8080"
	@echo "  make build-ipad   Build the native iPad app for Apple Silicon Simulator"
	@echo "  make build-ipad-device  Build the unsigned native iPad app for a device"
	@echo "  make install-linux  Install the app for the current user (Linux)"
	@echo
	@echo "Native dev controls: Ctrl+C restarts; Ctrl+\\ quits"

run: run-native

run-native: $(NATIVE_RUN_TARGET)

run-macos: sandbox-app
	@/usr/bin/open -W -n target/sandbox/app/Progred.app

run-linux:
	@./tools/run-linux $(ARGS)

install-linux:
	@./tools/install-linux-app

unsupported-native-platform:
	@echo "native Progred is not supported on $(UNAME_S)" >&2
	@exit 1

dev: dev-native

dev-native:
	@./tools/dev-native $(ARGS)

build-web: sandbox-web

run-web: build-web
	@echo "Open Progred: http://localhost:8080"
	@python3 -m http.server 8080 --bind 0.0.0.0 --directory web

# Compatibility aliases.
web: build-web

serve-web: run-web

build-ipad:
	@./tools/build-ipad simulator

build-ipad-device:
	@./tools/build-ipad device

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
