# Verification

[Verus](https://github.com/verus-lang/verus) proves properties of Progred's
Rust code: a function's specification states what it returns, and Verus, with
the Z3 solver, checks every input against it rather than sampled ones.
`make verify` runs it in the build sandbox. Today it proves the digit arithmetic
under `gid::position::between`; [deferred work](deferred.md#a-reference-grap-and-derivative-mode)
holds the larger plans.

## What is proven

In [`gid/src/position.rs`](../gid/src/position.rs), for canonical digits
(nonempty, with a nonzero final byte), `digits_between` returns `None` exactly
when both bounds are given and aren't strictly ordered, and otherwise canonical
digits strictly between its bounds. `after`, `before` and `bisect`, the cases it
chooses between, each prove the same of their results, and `in_order` decides
the order. Verus also proves that their loops end, that no arithmetic
overflows and that every index is in bounds. The order is lexicographic order on
the digits, `precedes`.

The specifications are the part to read. `canonical`, `precedes_at`, `precedes`
and `digit` define the terms; each function's `requires` says what it assumes and
its `ensures` what it guarantees. The rest is proof and the code itself.

## What is trusted

- Verus, Z3 and the Rust toolchain the release was built with.
- vstd's specifications of the standard library calls the code makes, such as
  `Vec::push`, `Vec::extend_from_slice` and slice indexing.
- The unverified code around the proof. `between` hands `digits_between` the
  digits of two `Position`s, relying on every `Position` being canonical:
  `from_bytes` checks it, `between`'s result is proven canonical, and `spread`
  trims its digits and checks them with `from_bytes`. `List` orders elements by
  `Position`'s derived `Ord`, and the standard library documents the slice
  ordering it uses as lexicographic, the order `precedes` states.
- `verus_builtin_macros`, which strips specifications and proofs from ordinary
  builds, so the code that runs is the code Verus checked without its ghost
  parts.

The proof uses no `assume`, `admit` or `external_body`, Verus's ways to accept
a claim unchecked. Once more code is verified, `make verify` should reject them
outside a reviewed list.

## Running it

```bash
rustup toolchain install 1.99.1 --profile minimal
```

```bash
make verify
```

Verus needs the toolchain its release was built with, named in
[`tools/verus-settings.sh`](../tools/verus-settings.sh). The first run downloads
that release from GitHub, about 450–490 MB, checks the archive against the
SHA-256 digest recorded there for the platform, and unpacks it, about 1.6 GB,
into `target/sandbox/verus`. It needs `curl` and `unzip`. Later runs are
offline and take about a second per verified file. Arguments to Verus go in
`ARGS`, such as `make verify ARGS='--expand-errors'`, which explains a failed
proof in more detail.

Run it after changing a verified file. Like `make test`, it runs under the
build sandbox. Verus's launcher would find its toolchain through rustup, which
the sandbox hides, so the wrapper runs it with `VERUS_USE_RUSTUP=0` and the
toolchain's library directory on the library path, as `rustup run` would.

## How builds see verified code

Verified code sits in a `verus!` block. Ordinary builds expand that macro from
`verus_builtin_macros`, pinned to the same release, which drops specifications,
proofs and loop invariants and leaves the code as written. The file imports
`vstd` only under `cfg(verus_only)`, which Verus sets while it verifies, so
ordinary builds don't depend on it.

Verus checks each listed file on its own, as a crate root, so a verified file
uses only the standard library; `position.rs` needs nothing more. Verifying
code that depends on other crates would move to `cargo verus`, with `vstd` as an
ordinary dependency.

Verified code is written the way Verus can follow: loops with invariants rather
than iterator chains, and standard library calls vstd specifies, such as
`extend_from_slice` and `push` in place of `to_vec` and `concat`.

## Updating Verus

Take a release at least seven days old, as for crates. In
`tools/verus-settings.sh`, update `verus_version`, `verus_toolchain` from the
release's `version.json`, and the three platforms' digests, taken from each
downloaded archive with `shasum -a 256`. In the root `Cargo.toml`, set the
`verus_builtin_macros` pin to the version in the release's
`source/builtin_macros/Cargo.toml`, record it with the wrapper's `resolve`, then
run `make verify` and `make test`.
