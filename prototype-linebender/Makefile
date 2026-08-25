RUSTC_PATH := $(shell rustup which rustc --toolchain stable)
CARGO_PATH := $(shell rustup which cargo --toolchain stable)

.PHONY: web serve-web

web:
	RUSTC="$(RUSTC_PATH)" "$(CARGO_PATH)" build --release -p progred --target wasm32-unknown-unknown
	wasm-bindgen --target web --no-typescript --out-dir web/pkg --out-name progred target/wasm32-unknown-unknown/release/progred.wasm

serve-web: web
	python3 -m http.server 8080 --bind 0.0.0.0 --directory web
