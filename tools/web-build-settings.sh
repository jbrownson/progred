web_toolchain=nightly-2026-08-27
web_target=wasm32-unknown-unknown
web_build_std=std,panic_abort
web_rustflags='-C target-feature=+atomics,+bulk-memory,+simd128 -C link-arg=--shared-memory -C link-arg=--max-memory=2147483648 -C link-arg=--import-memory -C link-arg=--export=__heap_base -C link-arg=--export=__wasm_init_tls -C link-arg=--export=__tls_size -C link-arg=--export=__tls_align -C link-arg=--export=__tls_base'
# The linked browser binary every browser build produces, and how
# wasm-bindgen wraps it for the page.
web_cargo_args='--release --bin progred -p progred'
web_bindgen_args='--target web --no-typescript --out-dir web/pkg --out-name progred'
# The fallback for pages without cross-origin isolation: no shared memory, so
# no workers. It rebuilds std too, so it needs nothing the threaded build doesn't.
web_fallback_rustflags='-C target-feature=+bulk-memory,+simd128'
web_fallback_bindgen_args='--target web --no-typescript --out-dir web/pkg-fallback --out-name progred'
