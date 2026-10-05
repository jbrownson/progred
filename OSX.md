# On the next macOS session

Web builds stay on macOS, under Seatbelt. Do not add a Linux browser build.

After Rust 1.100 is stable (12 November 2026), `sandbox-cargo update` and
`resolve` can use stable Cargo. `registry.global-min-publish-age` no longer
needs `-Z min-publish-age`. The browser build still needs a nightly for
`-Z build-std`. That flag has no scheduled stabilization, so leave
`nightly-2026-08-27` in place for `web-threaded`.
