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

The usual interactive development command is:

```sh
make run
```

It replaces `cargo run --release`: it performs the isolated release build,
packages and ad-hoc signs `Progred.app`, launches a fresh instance through
Launch Services with the App Sandbox entitlement, and waits for the app to
exit. Launch Services does not safely attach the sandboxed GUI process to the
invoking terminal's standard streams; use macOS logging when diagnostics are
needed.

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
That command enables network access and write access to `Cargo.lock` only for
the update; subsequent compilation remains offline and source-read-only.

`sandbox-fetch` downloads the locked dependency graph into a Cargo home under
`target/sandbox`. It has network access but remains filesystem-isolated;
fetching does not compile crates or execute their build scripts. The remaining
commands run Cargo under `sandbox-exec` with:

- no network access;
- an empty environment, apart from an explicit compiler toolchain and build
  variables;
- no read access to the user's real home directory;
- read-only access to the source tree; and
- write access only beneath `target/sandbox`.

The isolated target directory is deliberate. A compromised native artifact
must not be left in the ordinary `target` directory for a later unsandboxed
`cargo run` to execute.

The checked-in Cargo tripwire is defense against mistakes, not a security
boundary: a process running as the repository owner can override Cargo config
or edit the repository. Seatbelt (or a VM) is the boundary. In particular, do
not bypass the tripwire with `RUSTC_WRAPPER=` and then reuse those artifacts.

Cargo configuration cannot redirect `cargo run` itself because aliases may not
replace Cargo's built-in commands. Doing that transparently would require a
shell-level `cargo` shim ahead of the real Cargo executable on `PATH`; the
repository deliberately does not modify the user's global Rust tooling.

`sandbox-app` packages the isolated release binary as
`target/sandbox/app/Progred.app`, enables Apple's App Sandbox, and signs it
ad hoc for local use. Its only additional capability is read/write access to
files explicitly chosen through the app's native Open and Save panels. Launch
that bundle, rather than using `cargo run`, to retain the runtime sandbox:

```sh
open target/sandbox/app/Progred.app
```

The current command-line document path is not a user-selected file grant, so
open external documents through the app's File > Open command. The fixed
bundle identity gives macOS one stable app container, but the ad-hoc signature
is for local development only; distribution needs a Developer ID or App Store
signature and the usual notarization or review workflow.

`sandbox-exec` is a deprecated macOS facility, so this is a useful additional
build-time boundary rather than a permanent security architecture. The signed
app adds the supported runtime boundary; a VM remains stronger isolation for
truly hostile code.
