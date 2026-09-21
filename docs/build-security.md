# Build security

Cargo build scripts and procedural macros execute native code while a crate is
compiled. A locked dependency graph makes dependency changes visible, but does
not make compilation of those dependencies safe.

On macOS, use the repository's isolated build workflow for newly fetched or
otherwise untrusted dependencies:

```sh
make sandbox-fetch
make sandbox-check
make sandbox-build
make sandbox-test
make sandbox-app
make sandbox-web
```

The usual native development command is:

```sh
make run
```

On macOS it replaces `cargo run --release`: it performs the isolated release
build, packages and ad-hoc signs `Progred.app`, launches a fresh instance
through Launch Services with the App Sandbox entitlement, and waits for the app
to exit. Launch Services does not safely attach the sandboxed GUI process to
the invoking terminal's standard streams; use macOS logging when diagnostics
are needed.

`make dev` keeps an interactive development session open: Ctrl+C stops its
current build or app, rebuilds, and launches again; Ctrl+\ quits the session.
Quitting the app (Cmd+Q on macOS) also rebuilds and restarts it. A failed
command waits for Ctrl+C instead of retrying automatically.
Closing the terminal stops the session. Restarting discards unsaved changes in
that development instance, as terminating the previous development loop did.

The helper owns a process group for each command. On macOS, its small native
launcher opens the signed bundle through `NSWorkspace` and retains the returned
`NSRunningApplication`, so termination targets that instance rather than every
process named `progred`. Ordinary `make run` still uses `open -W -n`.
The loop lives outside Make recipes, so `make -n dev` only prints the command.
Run `python3 tools/test-dev-native.py` to check its process and terminal behavior
using disposable subprocesses without launching Progred.

On Linux, the same target delegates to `tools/run-linux`, which performs an
ordinary locked Cargo run in `target/native`. This is not sandboxed. The
checked-in launcher is the deliberate Linux exception to the Cargo tripwire;
do not reproduce its `RUSTC_WRAPPER` override in ad-hoc commands.

`website/build-ci.sh` is a second explicit exception, for publishing from a
disposable x86_64 Linux CI runner with `CI=true`. It compiles only the browser
binary using the locked dependencies and shared browser build settings, into
`target/ci-web`. It installs pinned tooling and has network access; it does not
provide a sandbox within the runner. Dependency build scripts remain trusted
within that CI environment. The OS/CI checks prevent accidental local use, not
malicious bypass. Local macOS builds continue through Seatbelt. See the
[website setup](../website/README.md#publishing-on-cloudflare).

The repository also contains `.cargo/config.toml` as an accidental-use
tripwire. Ordinary `cargo build`, `check`, `test`, and `run` commands stop at a
compiler wrapper instead of compiling dependency code, use a deliberately
separate target directory, and default Cargo to offline mode. This applies in
new terminals and automated coding sessions because Cargo discovers the file
from the repository itself. `make web` also delegates to `sandbox-web`.

The tripwire also blocks Cargo operations such as full dependency metadata
resolution when Cargo probes the compiler. Use `./tools/sandbox-cargo
metadata ...` when that information is needed; IDE integration may need to be
pointed at the sandbox wrapper rather than invoking Cargo directly.

Use `./tools/sandbox-cargo update ...` for an intentional dependency update.
That command uses the pinned `nightly-2026-08-27` Cargo solely for its
minimum-publish-age resolver, excludes registry releases less than seven days
old, and enables network access and write access to `Cargo.lock` only for the
update. Install that toolchain with `rustup toolchain install
nightly-2026-08-27 --profile minimal` if needed. Subsequent compilation remains
on stable Cargo for native targets, offline, and source-read-only.
The browser build is an explicit exception: `web-threaded` uses the same pinned nightly to
rebuild the standard library with atomics and enable WASM SIMD, under the same
Seatbelt restrictions, in a separate `target/sandbox/build-web` directory.
`web-threaded-fetch` only fetches
that toolchain's locked sysroot dependencies; it does not update the workspace
lockfile. `make build-web` uses this path; native builds remain on stable. The
pinned nightly needs `rust-src` and `llvm-tools`. The standalone
[worker diagnostic](web-worker-experiment-2026-09-19.md) uses the same build and
transport without starting the editor.
The browser Rayon pool uses `wasm-bindgen-rayon` 1.3.0 with `no-bundler`.
Its three added registry packages (`wasm-bindgen-rayon`, `wasm_sync`, and
`crossbeam-channel`) resolved under the same seven-day publication-age policy;
unrelated locked versions were retained. Browser WebGPU uses the existing
Vello/wgpu dependency graph and shaders, not new native build tooling.
The advisory scan after these additions found no new advisories, retaining
the six existing warnings described below.
Use `./tools/sandbox-cargo resolve` after declaring a new dependency when the
existing locked versions should be preserved; it uses the same publication-age
policy and network/lockfile boundary without compiling dependency code.
If a dependency has no age-compatible resolution, review the exact release and
its provenance before temporarily resolving with
`resolver.incompatible-publish-age="allow"`. Do not put that override in the
checked-in default: the ordinary update command should remain strict and may be
blocked until the reviewed release ages past the threshold.
On 2026-08-28, adding Fidget 0.5.0 required that exception for `chacha20`
0.10.2: Fidget's `rand` requirement cannot resolve to the yanked 0.10.0 or
0.10.1 releases. The 0.10.2 crate was checked against the signed release in the
official RustCrypto repository; it has no build script and contains a focused
SIMD feature-detection correction. Unrelated too-new transitive releases were
downgraded before checking in the lockfile.
Use `./tools/sandbox-cargo audit` to check the lockfile against the current
RustSec advisory database. The first run installs the pinned `cargo-audit`
version into the sandbox Cargo home; both that installation and every scan run
inside the same filesystem and network boundary.

### Reviewed Fidget development revision

On 2026-09-13, Fidget was pinned to upstream commit
[`0c89e87`](https://github.com/mkeeter/fidget/commit/0c89e87e1b3a6d15cc0976ab6ff05a09f9cf91d6)
(2026-09-12) for its per-object GPU color support, not a local rendering patch.
The upstream changelog calls the next release **0.5.1 (unpublished)**; replace
the Git pin with a reviewed published release when available.

Review covered the 42-commit delta from 0.5.0, commit provenance, changed
manifests/build scripts, and current advisory/news checks. This is a reviewed
exception to the age preference for the Git revision; registry resolution kept
the seven-day minimum and existing unrelated versions. Git uses upstream's real
`workspace-hack` development dependency unifier, unlike its empty published
shim, so it brings extra dependencies/features into the lockfile. There is no
local patch to suppress that helper. Cargo's built-in Git transport hit a TLS
error here; `./tools/sandbox-cargo resolve --config net.git-fetch-with-cli=true`
used the system Git transport within the same sandbox.

The RustSec scan had no vulnerability errors, but retained warnings for the
existing `im`, `sized-chunks`, `bitmaps`, and `ttf-parser` versions, including
unsoundness advisories for the first two. This review does not establish that
the dependency graph is safe; sandboxed builds remain required.

On 2026-09-15, two packages from that same revision were
[vendored with local patches](../vendor/README.md): large-tape AArch64 JIT fixes
and finer-grained raster cancellation. No other Fidget source was changed.
JIT is enabled only for Apple Silicon macOS implicit rasterization; meshing and
other platforms retain VM evaluation. Registry additions for `dynasmrt` and its
assembler dependencies resolved with the seven-day minimum; unrelated locked
versions were retained. A fresh RustSec scan found no new advisories, retaining
the six existing warnings described above. The JIT build script only checks CPU
features; compilation still runs inside Seatbelt.

The macOS bundle now declares Apple's
[allow-JIT entitlement](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.cs.allow-jit)
for Fidget's `MAP_JIT` mappings and per-thread write/execute protection. It does
not allow arbitrary unsigned executable memory or disable hardened runtime or
App Sandbox. The 196 headless JIT tests were also exercised in a minimal signed
bundle with the same entitlements; this is not a GUI interaction test.

### Sandbox boundaries

`sandbox-fetch` downloads the locked dependency graph into a Cargo home under
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

The isolated macOS target directory is deliberate. A compromised native
artifact must not be left in the ordinary Cargo target directory for a later
unsandboxed command to execute. The explicitly unsandboxed Linux launcher uses
the separate `target/native` directory.

The checked-in Cargo tripwire is defense against mistakes, not a security
boundary: a process running as the repository owner can override Cargo config
or edit the repository. Seatbelt (or a VM) is the boundary. Outside the
checked-in Linux launcher and hosted website build, do not bypass the tripwire
with `RUSTC_WRAPPER=` and then reuse those artifacts.

Cargo configuration cannot redirect `cargo run` itself because aliases may not
replace Cargo's built-in commands. Doing that transparently would require a
shell-level `cargo` shim ahead of the real Cargo executable on `PATH`; the
repository deliberately does not modify the user's global Rust tooling.

`sandbox-app` packages the isolated release binary as
`target/sandbox/app/Progred.app`, enables Apple's App Sandbox, and signs it
ad hoc for local use. Its additional capabilities are read/write access to
files explicitly chosen through the app's native Open and Save panels, and
JIT-compiled code as described above. Launch
that bundle, rather than using `cargo run`, to retain the runtime sandbox:

```sh
open target/sandbox/app/Progred.app
```

The current command-line document path is not a user-selected file grant, so
open external documents through the app's File > Open command. The fixed
bundle identity gives macOS one stable app container, but the ad-hoc signature
is for local development only; distribution needs a Developer ID or App Store
signature and the usual notarization or review workflow.

As of August 2026, Winit 0.30.13 is the latest non-beta release and Progred's
current version. A sandbox-authorized shell open would need Launch Services to
deliver `application:openURLs:` rather than passing the path in `argv`, but
Winit 0.30 owns the application delegate and panics if it is replaced. This is
tracked in [#4015](https://github.com/rust-windowing/winit/issues/4015) and the
still-open documentation bug
[#4458](https://github.com/rust-windowing/winit/issues/4458). Winit 0.31 beta
removes its custom macOS delegate so applications can install their own. Revisit
shell document opening when 0.31 is stable or an upgrade is otherwise useful;
do not patch the Objective-C runtime around 0.30 for this convenience.

`sandbox-exec` is a deprecated macOS facility, so this is a useful additional
build-time boundary rather than a permanent security architecture. The signed
app adds the supported runtime boundary; a VM remains stronger isolation for
truly hostile code.
