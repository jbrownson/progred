UNAME_S := $(shell uname -s)

ifeq ($(UNAME_S),Darwin)
NATIVE_RUN_TARGET := run-macos
else ifeq ($(UNAME_S),Linux)
NATIVE_RUN_TARGET := run-linux
else
NATIVE_RUN_TARGET := unsupported-native-platform
endif

.PHONY: help run run-macos run-linux install-linux unsupported-native-platform dev run-web run-website build-web build-website images sandbox-fetch sandbox-check sandbox-build sandbox-test sandbox-app

help:
	@echo "Development:"
	@echo "  make run          Run the native app once"
	@echo "  make dev          Rebuild after app exit or Ctrl+C"
	@echo "  make run-web      Serve the editor in a browser on port 8080"
	@echo "  make run-website  Serve the website with its lessons on port 8081"
	@echo "  make build-web    Build the browser editor (the servers build it first)"
	@echo "  make build-website  Package the publishable website into target/website"
	@echo "  make images       Regenerate the README and website screenshots from the editor"
	@echo "  make install-linux  Install the app for the current user (Linux)"
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

run-website:
	@./website/Preview.command $(ARGS)

# Headless captures of the real editor, rasterized by librsvg's rsvg-convert.
# The CAM capture waits for its progressive render, so it needs --release.
images:
	@command -v rsvg-convert >/dev/null || { echo "make images needs rsvg-convert (brew install librsvg)" >&2; exit 1; }
	./tools/sandbox-cargo test --release -p progred --lib -- --ignored readme_svg_captures website_preview_image website_libraries_image
	rsvg-convert target/sandbox/build/readme_cam.svg -o docs/images/cam-preview.png
	rsvg-convert target/sandbox/build/readme_iop.svg -o docs/images/iop-tree.png
	cp docs/images/cam-preview.png docs/images/iop-tree.png website/public/images/
	rsvg-convert -z 2 target/sandbox/build/website_preview.svg -o website/public/images/forest-light.png
	rsvg-convert -z 2 target/sandbox/build/website_libraries.svg -o website/public/images/libraries-light.png

# The launcher builds editor changes before serving.
run-web:
	@echo "Open Progred: http://127.0.0.1:8080/editor/"
	@python3 website/preview.py --no-open --port 8080

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

# Cargo skips unchanged builds; wasm-bindgen reruns only for a newer binary,
# using the release matching Cargo.lock.
build-web:
	./tools/sandbox-cargo web-threaded build --release --bin progred -p progred
	@wasm=target/sandbox/build-web/wasm32-unknown-unknown/release/progred.wasm; \
	if [ ! -f web/pkg/progred_bg.wasm ] || [ ! -f web/pkg/progred.js ] || [ "$$wasm" -nt web/pkg/progred_bg.wasm ]; then \
		./tools/wasm-bindgen --target web --no-typescript --out-dir web/pkg --out-name progred "$$wasm"; \
	fi
