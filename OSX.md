# On the next macOS session

Web builds stay on macOS, under Seatbelt. Do not add a Linux browser build.

`website/build-ci.sh` downloads the wasm-bindgen CLI, checks its SHA-256, and
unpacks it into `target/ci-tools`. `tools/wasm-bindgen` already does that for
the lockfile's version, including the Linux musl build, and `make build-web`
already calls it. Delete the copy in `build-ci.sh` and call `tools/wasm-bindgen`.
Confirm with `make build-web` on macOS, then delete this note.

After Rust 1.100 is stable (12 November 2026), `sandbox-cargo update` and
`resolve` can use stable Cargo. `registry.global-min-publish-age` no longer
needs `-Z min-publish-age`. The browser build still needs a nightly for
`-Z build-std`. That flag has no scheduled stabilization, so leave
`nightly-2026-08-27` in place for `web-threaded`.
