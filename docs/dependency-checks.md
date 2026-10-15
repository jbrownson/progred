# Dependency checks

A weekly check for security advisories and available updates, since nothing
updates on its own. Log each check below, newest first. When the latest entry is
more than a week old, a Claude Code session-start hook,
[`tools/dependency-check-due`](../tools/dependency-check-due), tells the agent
to suggest one.

## The check

1. `make audit`: advisories for `Cargo.lock` and both npm lockfiles.
2. `make outdated`: Cargo updates that pass the seven-day cutoff, newer
   incompatible releases, and outdated npm packages.
3. Before applying an update, look at the packages it would add or remove, which
   `make outdated` lists, and at anything unexpected.
4. Check the Rust toolchain with `rustup check`, and the vendored Fidget patches
   against upstream (see [vendor/README.md](../vendor/README.md)).
5. Apply what's worth it through the update commands (see [build
   security](build-security.md)), then run `make test`, and `make perf` when the
   update could affect performance. Plan breaking upgrades separately.
6. Log the date, the commit, what turned up, and what was done or deferred.

## Log

- 2026-10-11 (`21a3216b`), the first logged check.
  - **Advisories:** Cargo reports only RUSTSEC-2026-0192, `ttf-parser`
    unmaintained, through winit; it waits on winit 0.31. The website's Wrangler
    4.136.1 brings `undici` and `sharp` advisories through miniflare; the
    update to 4.149.0 is due on 15 October, when it passes the cutoff. The
    tools package is clean.
  - **Done today:** Fidget 0.5.1 with wgpu 30, Vello 0.11, parley 0.11.1 and
    skrifa 0.44; `im` replaced by `imbl`; the npm policy, `tools/npm-update`,
    `make audit` and `make outdated`.
  - **Applied:** compatible Cargo updates to 46 packages, through the sandboxed
    `update`. New to the lockfile: `tokio` through `wasm-bindgen-futures`
    0.4.79, but only for Emscripten with an unstable flag, and `zlib-rs`, an
    optional `flate2` backend we don't enable, so neither is built;
    `miniz_oxide` 0.9, `flate2`'s default backend; and `smallvec` 2.0.0-beta.2,
    replacing the alpha Fidget's `facet` used. `make test` passes.
    `make perf` on the Linux VM shows no slowdowns. The stock canaries ran 5–21%
    faster than the last record from the same host, but that's within the VM's
    run-to-run noise, so it isn't attributed to the updates.
  - **Deferred:** breaking upgrades to parley 0.12 (past the cutoff on 16
    October), pollster 1.0, getrandom 0.4, and base64 0.23 for tests; skrifa
    0.48 waits on Vello and parley, which require 0.44.
  - **Also today:** Linux builds now run in a namespace sandbox like the macOS
    one.
- 2026-10-15 (`553ecf64`), the Wrangler update due from the first check.
  - **Applied:** the website's Wrangler from 4.136.1 to 4.149.0, through
    `tools/npm-update`, which clears the `undici` and `sharp` advisories it
    brought through miniflare. `make audit` now reports only RUSTSEC-2026-0192.
    `npm ci` with install scripts off and a dry-run deploy both work.
