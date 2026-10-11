UNAME_S := $(shell uname -s)

# macOS builds under Seatbelt; Linux has no repository sandbox yet.
ifeq ($(UNAME_S),Darwin)
NATIVE_RUN_TARGET := run-macos
NATIVE_CARGO := ./tools/sandbox-cargo
NATIVE_TARGET_DIR := target/sandbox/build
else ifeq ($(UNAME_S),Linux)
NATIVE_RUN_TARGET := run-linux
NATIVE_CARGO := ./tools/linux-cargo
NATIVE_TARGET_DIR := target/native
else
NATIVE_RUN_TARGET := unsupported-native-platform
NATIVE_CARGO = $(error native Progred is not supported on $(UNAME_S))
endif

.PHONY: help run run-macos run-linux install-linux unsupported-native-platform dev run-web build-web build-website images fetch check build test test-vendor test-gpu perf sandbox-app

help:
	@echo "Development:"
	@echo "  make run          Run the native app once"
	@echo "  make dev          Rebuild after app exit or Ctrl+C"
	@echo "  make run-web      Serve the website and its editor on port 8081"
	@echo "  make build-web    Build the browser editor (the servers build it first)"
	@echo "  make build-website  Package the publishable website into target/website"
	@echo "  make images       Regenerate the README and website screenshots from the editor"
	@echo "  make install-linux  Install the app for the current user (Linux)"
	@echo "  make fetch        Download the locked dependencies"
	@echo "  make check        Type-check the workspace"
	@echo "  make build        Build the workspace"
	@echo "  make test         Test the workspace, except the vendored Fidget packages"
	@echo "  make test-vendor  Test the vendored Fidget packages"
	@echo "  make test-gpu     Run the tests that need a GPU"
	@echo "  make test-web-gpu Test the browser presentation in the installed Chrome"
	@echo "  make perf         Run the performance canaries and compare with the log"
	@echo
	@echo "Native dev controls: Ctrl+C restarts; Ctrl+\\ quits"

run: $(NATIVE_RUN_TARGET)

run-macos: sandbox-app
	@/usr/bin/open -W -n target/sandbox/app/Progred.app

run-linux:
	@./tools/run-linux $(ARGS)

install-linux:
	@./tools/install-linux-app

unsupported-native-platform:
	@echo "native Progred is not supported on $(UNAME_S)" >&2
	@exit 1

dev:
	@./tools/dev-native $(ARGS)

build-website: build-web
	python3 -B website/package.py

# The launcher builds editor changes before serving.
run-web:
	@python3 website/preview.py $(ARGS)

# Headless captures of the real editor, rasterized by librsvg's rsvg-convert.
# The CAM capture waits for its progressive render, so it needs --release.
images:
	@command -v rsvg-convert >/dev/null || { echo "make images needs rsvg-convert from librsvg" >&2; exit 1; }
	$(NATIVE_CARGO) test --release -p progred --lib -- --ignored readme_svg_captures website_preview_image website_libraries_image
	rsvg-convert $(NATIVE_TARGET_DIR)/readme_cam.svg -o docs/images/cam-preview.png
	rsvg-convert $(NATIVE_TARGET_DIR)/readme_iop.svg -o docs/images/iop-tree.png
	cp docs/images/cam-preview.png docs/images/iop-tree.png website/public/images/
	rsvg-convert -z 2 $(NATIVE_TARGET_DIR)/website_preview.svg -o website/public/images/forest-light.png
	rsvg-convert -z 2 $(NATIVE_TARGET_DIR)/website_libraries.svg -o website/public/images/libraries-light.png


fetch:
	$(NATIVE_CARGO) fetch

check:
	$(NATIVE_CARGO) check --workspace

build:
	$(NATIVE_CARGO) build --workspace

VENDORED := fidget-core fidget-raster fidget-wgpu

test:
	$(NATIVE_CARGO) test --workspace $(VENDORED:%=--exclude %)

# Upstream's tests of the vendored packages, optimized as vendor/README.md
# advises. Run them after changing a patch or upgrading Fidget. The binaries
# run outside the macOS build sandbox because fidget-wgpu's need a GPU.
test-vendor:
	$(NATIVE_CARGO) test --release --doc $(VENDORED:%=-p %)
	@python3 -B tools/test_binaries.py --release $(VENDORED:%=-p %)

# Ignored tests that need a GPU, run outside the macOS build sandbox.
test-gpu:
	@python3 -B tools/test_binaries.py --release -p puri-vello --features capture --test compositor -- --ignored --test-threads=1
	@python3 -B tools/test_binaries.py --release -p progred --lib -- --ignored --test-threads=1 compositor gpu

perf:
	@python3 -B tools/perf.py $(ARGS)

# The browser presentation's pixel test, run in the installed Chrome through
# the playwright-core that tools/package.json pins.
test-web-gpu:
	@set -e; . ./tools/web-build-settings.sh; \
	./tools/sandbox-cargo web-threaded build --release -p progred --example web_gpu_probe; \
	./tools/wasm-bindgen --target web --no-typescript --out-dir web/gpu-probe-pkg --out-name probe \
		target/sandbox/build-web/$$web_target/release/examples/web_gpu_probe.wasm; \
	[ -d tools/node_modules/playwright-core ] || npm ci --prefix tools; \
	node tools/test-web-gpu.cjs

sandbox-app:
	@./tools/build-macos-app

# Cargo skips unchanged builds; wasm-bindgen reruns only for a newer binary,
# using the release matching Cargo.lock.
build-web:
	@set -e; . ./tools/web-build-settings.sh; \
	./tools/sandbox-cargo web-threaded build $$web_cargo_args; \
	wasm=target/sandbox/build-web/$$web_target/release/progred.wasm; \
	if [ ! -f web/pkg/progred_bg.wasm ] || [ ! -f web/pkg/progred.js ] || [ "$$wasm" -nt web/pkg/progred_bg.wasm ]; then \
		./tools/wasm-bindgen $$web_bindgen_args "$$wasm"; \
	fi
