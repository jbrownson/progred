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
cells; the forms a record's `isa` makes it, calls, lambdas and value wrappers;
the closures lambdas come to; the absents all of these come to; the origins
closures and some absents carry, saying where code was written; fuel, which is
exact, one unit per expression evaluated as `Context::burn` documents; and how
deeply evaluations nest, past which evaluation halts. With it come the number
operations Grap runs natively, which the numbers library registers as
primitives. A program that calls any other foreign function leaves the core,
and the definition says so rather than guessing, so the rest of the libraries'
functions aren't defined yet.

The definition reads GID values as they mean, not as they're stored: a record,
and so an environment, maps cells to values, and a list is its elements in
order, since positions don't take part in a value's equality. A record is a
form only when its `isa` names one, and any other record is data, whatever
fields it has. A closure is the record it materializes as, `{isa: closure,
lambda, environment, body origin}`, holding its lambda as written, so the
definition never needs a second notion of value: calling a closure reads that
record back, as calling one written as data does. A host gives each cell a
value, which referencing the cell evaluates, and may implement a foreign
function under it, which calling the cell runs when its value is a foreign
function implemented under that cell. An origin is a path, as GID's path data
writes one, from the input an evaluation was handed or from a cell's
definition. Evaluation reports the fuel it spent, and callers pass on only
what's left, which makes the definition's termination visible to Verus.

The number operations are the library's: f64 and f32 arithmetic and
comparisons, f64's `sin`, `cos`, `floor`, `ceil` and `is_finite`, and u64's
arithmetic, which fails rather than wrapping. A call to one evaluates its
operands in label order and reads each as a number of the operation's type, a
record whose tag holds exactly that type's bytes, whatever else it holds. When
one isn't, the call comes to "invalid arguments", naming where the call was
written and carrying why under each label, by the rule Grap's
`absent::caused_by_all` follows: an absent the same as one before it isn't
repeated, and when the longest chain of causes among them reaches the limit,
that cause passes on as it is rather than growing.

u64 arithmetic is defined exactly. The float operations are named rather than
defined, as Rust computes them on the platform the evaluator runs on: IEEE 754
defines most of their results, but Rust leaves a NaN's sign and payload, which
of two equal zeros `min` and `max` answer, and how closely `hypot`, `sin` and
`cos` round to the platform. The definition holds Grap to whatever Rust
computes for them.

[`evaluator.rs`](../grap-reference/src/evaluator.rs) is the same evaluator as
code that runs, and Verus proves it computes exactly what the definition says
for every program, host and budget: result, fuel left and halt. `value.rs`
holds the model and the cells Grap's vocabulary names, and `absent.rs`,
`number.rs` and `origin.rs` the evaluators' absents, numbers and origins.
`absent.rs` compares chains of causes without counting them, so no count
overflows, however deep an absent a program writes.

[`fast.rs`](../grap-reference/src/fast.rs) is a second evaluator, proven the
same way, built the way a fast one is: expressions lowered once into nodes,
closures and numbers held as runtime values rather than records, environments
as shared frames rather than copies, host cells resolved when code is lowered,
each node's origin shared with the code within it rather than copied, and a
sorted set of every cell a lambda binds, so looking up any other cell skips
the frames. Its proof relates each structure to what it means, a runtime value
to the GID value it materializes as, frames to the environment they make,
lowered code to the expressions it came from and where they were written, and
shows each step does what the definition's does to those meanings. On the
timing programs in `grap/src/tests/reference.rs` it is faster than Grap's
evaluator on four and as fast on the fifth;
[deferred work](deferred.md#a-reference-grap-and-derivative-mode) has the
numbers.

The definition is the part to read, and it records behaviours the evaluator
has that nothing else had written down:

- Calling a cell spends a unit before its definition is evaluated, and calling
  a function bound in the environment spends one for the function, but
  neither nests an evaluation, so neither halts where evaluations nest too
  deeply; only the evaluations they lead to can.
- A form lacking a field its kind requires is why it's missing where the field
  would be: a call without a function, a value wrapper without a value and a
  lambda without a body come to `{isa: absent, reason: missing field, cell}`,
  for the unit evaluating the field would cost. A closure holds that absent
  as its lambda's body.
- A closure written as data whose body origin doesn't read as an origin is
  called with its body written nowhere, so a call in its body names no call in
  its absents; one whose origin reads is written back as it reads, in GID's
  canonical form.

Writing the definition found one place Grap's evaluator didn't do what it
documented: reading a number operand, it skipped evaluating a literal or bound
f64 but also the depth check that evaluation makes, so an operand at the depth
limit answered rather than halting. It now halts there, as
`Context::eval_f64` says it matches `Context::eval`.

Grap's own tests, in [`grap/src/tests/reference.rs`](../grap/src/tests/reference.rs),
check its real evaluator against both proven ones: on the core cases of
`conformance.rs`, with their pinned results and fuel; on what records say they
are, what forms lacking fields come to, a closure's shape and the origins
absents name; on the behaviours above; and on 20,000 generated programs, about
19,700 of which stay in the core. They register the number operations as the
numbers library does, and check every one on every pair from a spread of
operands, NaNs with payloads, infinities, zeros of both signs, subnormals,
u64's limits and values of the wrong type or none, with the invalid-arguments
rule's cases pinned and an operand at the depth limit; on 20,000 more
generated programs that call them, about 19,700 of which stay within the
definition; and on 4,000 that enter definitions calling each other, most with
budgets large enough to nest too deeply. They compare results whole, origins
included, and a float NaN as any NaN, since Rust leaves its sign and payload
unspecified. When the evaluator changes what the core does, these tests fail,
and either the definition changes with it, deliberately, or the evaluator is
wrong.

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
offline: `position.rs` takes about a second, `grap-reference` about twenty. Arguments to Verus go in
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
