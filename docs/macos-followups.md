# macOS follow-ups

Changes from the October 2026 cloud sessions that were built and tested on
Linux only.
Check each on macOS, then delete it from this list; delete the file when it's
empty.

## Next

- **Perf on AC power, with the JIT and without.** The first Mac baseline ran on
  battery. Its `stock_render`, 5.6 s, was the only canary slower than the 4-core
  Linux VM's (4.2 s), and it's the only one that uses the JIT. On AC power,
  record a default run, then one with the VM, which compares with the first:

  ```sh
  make perf ARGS='--record "Mac baseline on AC power"'
  make perf ARGS='--features software-vm --record "Software rendering on the VM, not the JIT"'
  ```

  The JIT branch of the switch, in `progred/src/libraries/fidget/raster.rs`,
  only compiles on Apple Silicon, so these runs are its first build. If the VM
  wins on the stock, choose the evaluator by program size rather than platform.
- **npm.** `playwright-core` 1.63.0 moved to `tools/package.json`, and both npm
  packages now turn off install scripts and require week-old releases in
  `.npmrc`. Check that `npm --version` is 11.10 or later, since older npm
  ignores `min-release-age`. Then run `make test-web-gpu`, and in `website/`,
  `npm ci` and `npx wrangler --version`.

## Later

- **Rust 1.100, due 12 November 2026.** Move `sandbox-cargo update` and
  `resolve` to stable Cargo, where `registry.global-min-publish-age` no longer
  needs `-Z min-publish-age`, and merge their near-identical branches. See
  [build security](build-security.md).
