# Verification

[Verus](https://github.com/verus-lang/verus) proves properties of Progred's
Rust code: a function's specification states what it returns, and Verus, with
the Z3 solver, checks every input against it rather than sampled ones.
`make verify` runs it in the build sandbox. Today it proves the digit arithmetic
under `gid::position::between`, and an evaluator that computes exactly what
Grap's core is defined to mean; [deferred work](deferred.md#a-reference-grap-and-derivative-mode)
holds the larger plans.

## What is proven

### List positions

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

### Grap's core

[`grap-reference`](../grap-reference/src/definition.rs) defines what Grap's core
means: a slow, simple evaluator written as Verus specification functions, the
law of how the core works. The core is what Grap's evaluator does itself:
cells, lambdas, calls, value wrappers, closures, the absents those come to, and
fuel, which is exact, one unit per expression evaluated as `Context::burn`
documents. A program that calls a foreign function leaves the core, and the
definition says so rather than guessing, so the libraries' functions aren't
defined yet.

The definition reads GID values as they mean, not as they're stored: a record,
and so an environment, maps cells to values, and a list is its elements in
order, since positions don't take part in a value's equality. A closure is the
record it materializes as, `{closure: {params, body, ..., environment}}`, so
the definition never needs a second notion of value. Evaluation reports the
fuel it spent, and callers pass on only what's left, which makes the
definition's termination visible to Verus.

[`evaluator.rs`](../grap-reference/src/evaluator.rs) is the same evaluator as
code that runs, and Verus proves it computes exactly what the definition says
for every program, host and budget: result, fuel left and completion.
`value.rs` holds the model and the cells Grap's vocabulary names.

The definition is the part to read, and it records two behaviours the
evaluator has that nothing else had written down:

- Calling a cell whose definition comes to an absent answers that absent as it
  is, while calling a binding to one answers that it isn't callable.
- A reference notices a cycle only through another reference to the same cell,
  while a call notices one through anything under resolution, so calling a cell
  defined as a reference to itself evaluates its definition twice first.

Grap's own tests, in [`grap/src/reference_tests.rs`](../grap/src/reference_tests.rs),
check its real evaluator against the proven one: on the core cases of
`conformance.rs`, with their pinned results and fuel, on the behaviours above,
and on 20,000 generated programs, about 19,500 of which stay in the core. The
comparison leaves out the source origins closures carry for click-to-source,
which the core doesn't define yet. When the evaluator changes what the core
does, these tests fail, and either the definition changes with it, deliberately,
or the evaluator is wrong.

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
- For Grap's core, that the definition says what Grap should mean. The proof
  ties the reference evaluator to it, and the tests tie Grap's evaluator to the
  reference evaluator, on the programs they try rather than all of them.

The proofs use no `assume`, `admit` or `external_body`, Verus's ways to accept
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

Specification-only types, such as the definition's `Value`, which holds vstd's
`Seq` and `Map`, carry `cfg(verus_only)` too: nothing that runs refers to them,
so ordinary builds compile the same code Verus checked. Verus warns that using
the flag on code that runs could make a proof say nothing about the program, so
it stays on imports and specification-only items.

Verus checks each listed file on its own, as a crate root, so verified code
uses only the standard library: `position.rs` needs nothing more, and
`grap-reference` depends on nothing but the macros. Verifying code that depends
on other crates would move to `cargo verus`, with `vstd` as an ordinary
dependency.

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
