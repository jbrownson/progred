# macOS follow-ups

Changes from the October 2026 cloud sessions that were built and tested on
Linux only.
Check each on macOS, then delete it from this list; delete the file when it's
empty.

## Next

- **`make audit`.** Added after the npm check: it runs `sandbox-cargo audit`
  and `npm audit` for both npm packages, reporting them all before failing.
  The website's Wrangler advisories are expected until the update due on
  15 October.
- **wasm-bindgen 0.2.129.** The lockfile update moved it from 0.2.127, so
  `make build-web` downloads the matching CLI. Check that the browser editor
  still runs with `make run-web`.

## Later

- **Rust 1.100, due 12 November 2026.** Move `sandbox-cargo update` and
  `resolve` to stable Cargo, where `registry.global-min-publish-age` no longer
  needs `-Z min-publish-age`, and merge their near-identical branches. See
  [build security](build-security.md).
