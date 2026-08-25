.PHONY: run sandbox-fetch sandbox-check sandbox-build sandbox-test sandbox-app sandbox-web web serve-web

run: sandbox-app
	/usr/bin/open -W -n target/sandbox/app/Progred.app

sandbox-fetch:
	./tools/sandbox-cargo fetch

sandbox-check:
	./tools/sandbox-cargo check --workspace

sandbox-build:
	./tools/sandbox-cargo build --workspace

sandbox-test:
	./tools/sandbox-cargo test --workspace

sandbox-app:
	./tools/build-macos-app

sandbox-web:
	./tools/sandbox-cargo build --release -p progred --target wasm32-unknown-unknown
	wasm-bindgen --target web --no-typescript --out-dir web/pkg --out-name progred target/sandbox/build/wasm32-unknown-unknown/release/progred.wasm

web:
	$(MAKE) sandbox-web

serve-web: web
	python3 -m http.server 8080 --bind 0.0.0.0 --directory web
