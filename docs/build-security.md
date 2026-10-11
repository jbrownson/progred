# Build security

Cargo build scripts and procedural macros execute native code while a crate is
compiled. A locked dependency graph makes dependency changes visible, but does
not make compilation of those dependencies safe.

On macOS, the Make targets (`fetch`, `check`, `build`, `test`, `run`, `dev`,
`sandbox-app`, and `build-web`) compile under the repository's Seatbelt sandbox,
and `make run` launches the result as an App Sandbox bundle. On Linux, the
native targets compile in a namespace sandbox with the same boundaries; the app
itself runs outside it. Use `./tools/sandbox-cargo` or `./tools/linux-cargo`
`<cargo command> [arguments...]` for other Cargo commands. How each platform
builds and runs is in [platforms](platforms.md).

## Sandbox boundaries

On macOS, `make fetch` downloads the locked dependency graph into a Cargo home under
`target/sandbox`. It has network access but remains filesystem-isolated;
fetching does not compile crates or execute their build scripts. The remaining
commands run Cargo under `sandbox-exec` with:

- no network access;
- an empty environment, apart from an explicit compiler toolchain and build
  variables;
- no read access to the user's real home directory;
- read-only access to the source tree, excluding `.git`, local agent
  configuration, and a root `.env` file; and
- write access only beneath `target/sandbox`.

On Linux, `tools/linux-cargo` draws the same boundaries with unprivileged user,
mount, PID and network namespaces, and `tools/linux-sandbox` builds the view
inside them. It's an allowlist, like the Seatbelt profile: a new root in which
every host directory is mounted read-only; the home directory and `/run` empty
and read-only; `/tmp`, `/var/tmp` and `/dev/shm` empty scratch space that
vanishes with the sandbox; and a `/dev` with only the basic devices. The source
tree is read-only with `.git`, local agent configuration and a root `.env`
hidden, and only `target/sandbox` is writable. The command runs with no
capabilities, so it can't remount or escape any of this, without other processes
in view, and in a session of its own, so it can't reach the terminal. Builds are
offline. When the locked graph isn't all in the sandbox's Cargo home,
`linux-cargo` first fetches it with network access, which runs no dependency
code; steps with network access also get a proxy's settings, its certificate
bundle and the resolver configuration. The sandbox needs unprivileged user
namespaces, which some distributions restrict: Ubuntu does through AppArmor
(`kernel.apparmor_restrict_unprivileged_userns`).

The isolated target directory is deliberate. A compromised native artifact must
not be left in the ordinary Cargo target directory for a later unsandboxed
command to execute. Some commands do run what the sandbox built outside it: the
GPU tests, `make perf` and the vendored Fidget tests, which need the GPU, and on
Linux the app itself, through `make run` and `make install-linux`.

`sandbox-exec` is a deprecated macOS facility, so this is a useful additional
build-time boundary rather than a permanent security architecture. The signed
app adds the supported runtime boundary; a VM remains stronger isolation for
truly hostile code. Linux namespaces are the kernel's supported isolation
mechanism, the one containers use, but share the same kernel.

## Tripwire

The repository contains `.cargo/config.toml` as an accidental-use tripwire. Ordinary `cargo build`, `check`, `test`, and `run` commands stop at a
compiler wrapper instead of compiling dependency code, use a deliberately
separate target directory, and default Cargo to offline mode. This applies in
new terminals and automated coding sessions because Cargo discovers the file
from the repository itself. `make build-web` runs the browser build through the same wrapper.

The tripwire also blocks Cargo operations such as full dependency metadata
resolution when Cargo probes the compiler. Use `./tools/sandbox-cargo
metadata ...` when that information is needed; IDE integration may need to be
pointed at the sandbox wrapper rather than invoking Cargo directly.

The checked-in Cargo tripwire is defense against mistakes, not a security
boundary: a process running as the repository owner can override Cargo config
or edit the repository. The sandboxes (or a VM) are the boundary. Outside the
two wrappers and the hosted website build, do not bypass the tripwire with
`RUSTC_WRAPPER=` and then reuse those artifacts.

Cargo configuration cannot redirect `cargo run` itself because aliases may not
replace Cargo's built-in commands. Doing that transparently would require a
shell-level `cargo` shim ahead of the real Cargo executable on `PATH`; the
repository deliberately does not modify the user's global Rust tooling.

## Exceptions

`website/build-ci.sh` is the explicit exception, for publishing from a
disposable x86_64 Linux CI runner with `CI=true`. It compiles only the browser
binary using the locked dependencies and shared browser build settings, into
`target/ci-web`. It installs pinned tooling and has network access; it does not
provide a sandbox within the runner. Dependency build scripts remain trusted
within that CI environment. The OS/CI checks prevent accidental local use, not
malicious bypass. Local builds continue through the sandboxes. See the
[website setup](../website/README.md#publishing-on-cloudflare).

## App Sandbox

`sandbox-app` packages the isolated release binary as
`target/sandbox/app/Progred.app`, enables Apple's App Sandbox, and signs it
ad hoc for local use. Its one additional capability is read/write access to
files explicitly chosen through the app's native Open and Save panels; it
declares no allow-JIT entitlement and maps no executable memory, since Fidget's
VM does the implicit rendering. Launch that bundle, as `make run` does, rather
than the bare binary, to keep the runtime sandbox.

The current command-line document path is not a user-selected file grant, so
open external documents through the app's File > Open command; opening them
from the shell is [deferred](deferred.md#opening-documents-from-the-macos-shell).
The fixed bundle identity gives macOS one stable app container, but the ad-hoc
signature is for local development only; distribution needs a Developer ID or
App Store signature and the usual notarization or review workflow.

## Dependency updates

Use `./tools/sandbox-cargo update ...` for an intentional dependency update.
That command uses the pinned `nightly-2026-08-27` Cargo solely for its
minimum-publish-age resolver, excludes registry releases less than seven days
old, and enables network access and write access to `Cargo.lock` only for the
update. On Linux, `./tools/linux-cargo update` applies the same policy with the
same toolchain and the same boundary. It's the toolchain the
browser build uses; see [its setup](../web/README.md). Subsequent compilation remains on stable Cargo
for native targets, offline, and source-read-only. Once Rust
1.100 is stable (12 November 2026), `update` and `resolve` can use stable
Cargo, where `registry.global-min-publish-age` no longer needs
`-Z min-publish-age`; the browser build keeps the nightly for `-Z build-std`.
The browser build is an explicit exception: `web-threaded`, which
`make build-web` uses, builds with the same pinned nightly to rebuild the
standard library with atomics, under the same Seatbelt restrictions, in a
separate `target/sandbox/build-web` directory. `web-threaded-fetch` only fetches
that toolchain's locked sysroot dependencies; it does not update the workspace
lockfile.

Use `./tools/sandbox-cargo resolve` after declaring a new dependency when the
existing locked versions should be preserved; it uses the same publication-age
policy and network/lockfile boundary without compiling dependency code. On
Linux, `./tools/linux-cargo resolve` records the change from the sandbox's Cargo
cache alone, offline, so it can add edges between already-locked packages but
cannot introduce a newly published release.
If a dependency has no age-compatible resolution, review the exact release and
its provenance before temporarily resolving with
`resolver.incompatible-publish-age="allow"`. Do not put that override in the
checked-in default: the ordinary update command should remain strict and may be
blocked until the reviewed release ages past the threshold.

`make audit` checks every lockfile against its advisory database: Cargo's
against RustSec, through the wrapper's `audit` command, and both npm packages'
against GitHub's. It reports them all before failing. The first `audit` installs
the pinned `cargo-audit` version into the sandbox's Cargo home; that
installation and every scan run inside the sandbox, with network access. The
weekly
[dependency check](dependency-checks.md) runs it along with `make outdated`.

## npm packages

JavaScript tooling lives in two npm packages, each with exact pins and its own
lockfile, so installing one never brings in the other's tree: `website/` holds
Wrangler for publishing, and `tools/` holds `playwright-core` for the browser
diagnostics. Install with `npm ci`, which follows the lockfile and its integrity
hashes exactly.

Each package's `.npmrc` follows the Cargo policy as far as npm allows:

- `ignore-scripts=true`: dependency install scripts, npm's counterpart of build
  scripts, don't run. Scripts named on the command line, such as `npm run
  build`, still do. With install scripts off, Wrangler still installs and a
  dry-run deploy works; the site is static assets, so deploying bundles no code.
- `min-release-age=7`: installs and updates resolve only releases at least seven
  days old. npm reads this from version 11.10 and older versions silently
  ignore it, so add or update dependencies with `tools/npm-update website|tools
  [package@version...]`, which passes the same cutoff as `--before` to older
  npm. Don't use `npm audit fix`: older npm applies no cutoff to it, and
  `--force` replaces the pinned versions.

npm has no sandbox like the Cargo wrappers': Node tooling runs with the invoking
user's permissions, as the tests do. `npm audit signatures` checks the installed
packages' registry signatures. Cloudflare's builder installs the website package
itself on each deploy.

## Current dependency exceptions

- **Patched Fidget.** Three Fidget packages are vendored from the published
  0.5.1 with local patches listed in [vendor/README.md](../vendor/README.md);
  Cargo uses them in place of the crates.io packages.

A review or a clean advisory scan doesn't establish that the dependency graph
is safe; sandboxed builds remain required. Earlier reviews are in
[history](history/dependency-reviews-2026.md).
