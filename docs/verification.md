# Verification

[Verus](https://github.com/verus-lang/verus) proves properties of Progred's
Rust code: a function's specification states what it returns, and Verus, with
the Z3 solver, checks every input against it rather than sampled ones.
`make verify` runs it in the build sandbox. Today it proves the digit arithmetic
under `gid::position::between`, and two evaluators that compute exactly what
Grap's core, number operations included, is defined to mean: a simple one, and
one built for speed. [Deferred work](deferred.md#a-reference-grap-and-derivative-mode)
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
documents; and the number operations it runs natively, which the numbers
library registers as primitives. A program that calls any other foreign
function leaves the core, and the definition says so rather than guessing, so
the rest of the libraries' functions aren't defined yet.

The definition reads GID values as they mean, not as they're stored: a record,
and so an environment, maps cells to values, and a list is its elements in
order, since positions don't take part in a value's equality. A closure is the
record it materializes as, `{closure: {params, body, ..., environment}}`, so
the definition never needs a second notion of value. Evaluation reports the
fuel it spent, and callers pass on only what's left, which makes the
definition's termination visible to Verus.

The number operations are the library's: f64 and f32 arithmetic and
comparisons, f64's `sin`, `cos`, `floor`, `ceil` and `is_finite`, and u64's
arithmetic, which fails rather than wrapping. A call to one evaluates its
operands in label order and reads each as a number of the operation's type, a
record whose tag holds exactly that type's bytes, whatever else it holds. When
one isn't, the call comes to "invalid arguments", carrying why under each label
by the rule Grap's `absent::caused_by_all` follows: an absent the same as one
before it isn't repeated, and when the longest chain of causes among them
reaches the limit, that cause passes on as it is rather than growing.

u64 arithmetic is defined exactly. The float operations are named rather than
defined, as Rust computes them on the platform the evaluator runs on: IEEE 754
defines most of their results, but Rust leaves a NaN's sign and payload, which
of two equal zeros `min` and `max` answer, and how closely `hypot`, `sin` and
`cos` round to the platform. The definition holds Grap to whatever Rust
computes for them.

[`evaluator.rs`](../grap-reference/src/evaluator.rs) is the same evaluator as
code that runs, and Verus proves it computes exactly what the definition says
for every program, host and budget: result, fuel left and completion.
`value.rs` holds the model and the cells Grap's vocabulary names, and
`absent.rs` and `number.rs` the evaluator's absents and numbers. `absent.rs`
compares chains of causes without counting them, so no count overflows,
however deep an absent a program writes.

[`fast.rs`](../grap-reference/src/fast.rs) is a second evaluator, proven the
same way, built the way a fast one is: expressions lowered once into nodes,
closures and numbers held as runtime values rather than records, environments
as shared frames rather than copies, host cells resolved when code is lowered,
and a sorted set of every cell a lambda binds, so looking up any other cell
skips the frames. Its proof relates each structure to what it means, a runtime
value to the GID value it materializes as, frames to the environment they
make, lowered code to the expressions it came from, and shows each step does
what the definition's does to those meanings. On the timing programs in
`grap/src/reference_tests.rs` it is faster than Grap's evaluator, which also
does what the definition leaves out, such as tracking click-to-source origins;
[deferred work](deferred.md#a-reference-grap-and-derivative-mode) has the
numbers.

The definition is the part to read, and it records two behaviours the
evaluator has that nothing else had written down:

- Calling a cell whose definition comes to an absent answers that absent as it
  is, while calling a binding to one answers that it isn't callable.
- A reference notices a cycle only through another reference to the same cell,
  while a call notices one through anything under resolution, so calling a cell
  defined as a reference to itself evaluates its definition twice first.

Grap's own tests, in [`grap/src/reference_tests.rs`](../grap/src/reference_tests.rs),
check its real evaluator against the proven ones: on the core cases of
`conformance.rs`, with their pinned results and fuel, on the behaviours above,
and on 20,000 generated programs, about 19,500 of which stay in the core. They
register the number operations as the numbers library does, and check every
one on every pair from a spread of operands, NaNs with payloads, infinities,
zeros of both signs, subnormals, u64's limits and values of the wrong type or
none, with the invalid-arguments rule's cases pinned, and on 20,000 more
generated programs that call them, about 19,500 of which stay within the
definition. The comparison leaves out the source origins closures carry for
click-to-source, which the core doesn't define yet, and compares a float NaN as
any NaN, since Rust leaves its sign and payload unspecified. When the evaluator
changes what the core does, these tests fail, and either the definition changes
with it, deliberately, or the evaluator is wrong.

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
- For the float operations, that `float_binary` and `float_unary` in
  [`number.rs`](../grap-reference/src/number.rs), which both evaluators call,
  compute what the definition names: they run Rust's operations on the
  operands' bits, in plain code beside them. Where Rust leaves a result to the
  platform, it lets it differ even between two places that run one operation,
  which the trust assumes it doesn't, and it does happen: one f64 operation on
  NaNs of both signs answered `+NaN` in Grap's evaluator and `−NaN` in the
  reference's. The tests compare NaNs as NaNs for that reason.

The proofs use no `assume` or `admit`, and `external_body`, Verus's way to
accept a function unchecked, only on those two float functions.
`loop_isolation(false)`, on a few functions, only lets a loop's body see what
was known before it, and `rlimit` only gives one proof more time. Once more code
is verified, `make verify` should reject them outside a reviewed list.

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
offline: `position.rs` takes about a second, `grap-reference` about fifteen. Arguments to Verus go in
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
