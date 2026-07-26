# Projections As Plugins

Decision record, 2026-07-25. How custom projections are defined, what
language they are written in, and how the host runs them.

## The Decision

Custom projections are **plugins**: programs in existing languages,
compiled to wasm, called by the editor as pure functions. The first
language is Rust. The split of ownership is the design's spine:

- **Structure is ours** — programs and documents live in the graph;
  text is a build artifact, like object code.
- **Semantics are external** — rustc owns what a program means. The
  editor never interprets, typechecks, or evaluates the language.
- **Execution is sandboxed** — wasmtime owns running it, with the
  capability surface chosen by the host.

Any language that compiles to wasm and can parse the gid notation can
write projections; the ABI is three exports and a byte protocol.

## Rejected: Grap, An Own Language

The path here went through designing Grap — a lisp whose
s-expressions are gid values: one `{is: head, …}` form, application
binding parameter *cells* by identity, records as templates, symbols
as cells, strict and fueled, projections as Grap functions. The
design was coherent and parts of it were genuinely better than the
textual equivalents (capture is unrepresentable when references are
edges; renaming is metadata).

It was abandoned at the design stage, before any evaluator code, when
each design session surfaced another hole that was ours to own
forever: resolution keyed on cell valuelessness (fragile — storage
state flipping reference semantics), derived signatures (conflated
symbols with parameters), `apply` arity, the macro/hygiene boundary
(fixed identities in templates reproduce the classic capture bug;
minting at instantiation is the eventual answer). Each was fixable;
the sum was a language project, and the tripwire — "Grap must stay
small; if it becomes a language project, that's the signal" — fired
during design. The editor is a big enough thing to explain without a
language to explain beside it.

What survives Grap's design work, language-independently: completions
as data carried on descend points (the projection is a grammar run in
the generative direction), doc reads as explicit tracked acts, the
string-tag dispatch convention, and the mint-on-instantiation note
for whenever code generation arrives.

## The Wasm Interface (ABI 1)

A plugin exports `abi_version() -> u32`, `alloc(len: u32) -> u32`,
`project(ptr: u32, len: u32) -> u64`, plus its linear `memory`. All
input crosses as bytes written into guest memory at a guest-allocated
offset; the reply is a packed pointer/length in the returned u64, and
**zero is the plugin declining the value** — the host falls through
to the ordinary rendering. Failure has one shape: the trap (panic,
out-of-bounds, deadline), caught at the call and rendered as an
error, never a crash.

- **A fresh Store per call.** No guest state survives an invocation,
  which makes `project` provably pure in its input bytes — and purity
  is what makes host-side memoization sound. Plugins run per novel
  value, not per frame.
- **Zero imports.** Purity by construction: nothing impure exists for
  the guest to call. The one planned future import is
  `resolve(cell) -> value` for following references — and because
  every call goes through the host, the resolve log is exactly the
  projection's dependency set: invalidation granularity falls out of
  the capability boundary.
- **Epochs, not fuel.** A watchdog thread ticks the engine's epoch;
  calls get a two-tick deadline and trap out if wedged. Fuel's
  deterministic accounting costs real instrumentation and answers a
  question nobody is asking yet.
- **Wire format**: v0 is task-specific bytes (the f64 plugin takes
  the eight blob bytes, returns UTF-8). v1 is **gid notation both
  directions** — the text format is the cross-language interface, no
  serde, no second Rust-only encoding. A binary sibling happens when
  profiling says text matters, specified like the text form.

## The Compile Service

Plugin Rust becomes wasm through a pinned toolchain over pipes:
source on stdin, module on stdout (`-o -`), diagnostics on stderr as
rustc's JSON with byte spans into the submitted source. No cargo in
the loop; no files on our side.

- **The toolchain is resolved through rustup by name, never bare
  `rustc`** — the PATH winner may lack the wasm target (Homebrew's
  does, which cost an afternoon's confusion).
- **Every call gets its own scratch directory under the system temp,
  serving as the child's cwd and its TMPDIR**: rustc stages stdout
  output as `stdout.<crate>` in its working directory (concurrent
  compiles sharing one clobber each other), and its intermediates
  follow TMPDIR — pointing both at one per-call directory contains
  the compiler's whole footprint, removed after the call. The system
  temp because **paths are environment policy, and dev must run the
  production shape**: an installed app is a read-only signed bundle
  whose writable roots are $TMPDIR and ~/Library — a checkout-
  relative scratch would work only in the environment that
  eventually goes away. (`temp_dir` honors $TMPDIR, the knob
  sandboxes redirect.) The spike's checkout-relative plugin source
  and wasm cache paths are dev-mode stand-ins awaiting plugin
  discovery, not precedent.
- The toolchain is a **runtime component of the editor**, not a dev
  assumption: rlib formats are compiler-version-specific, so the
  precompiled-dependency design requires one exact compiler the app
  owns. Tier 1 (now): pin and resolve explicitly, fail with the
  install one-liner. Tier 2 (when anyone else runs this):
  app-managed toolchain download. Keep the shared model crate free of
  proc-macro dependencies — derives execute host-side and drag host
  std into the minimal toolchain.
- rustc-as-a-library (`rustc_private`) was examined and declined: it
  pins the *app* to nightly, and isolating it in a helper binary
  reinvents the pipe architecture with a maintenance tax. The CLI is
  the stable API.

## The Rust Domain (Ahead)

Editing Rust as graph structure, emitted as text for the compiler:

- **Identifiers are gid-encoded** in emission (`g<hex>`), so renames
  never touch emitted text, duplicate names coexist, and shadowing is
  unrepresentable — the hygiene discussion's conclusion applied at
  the one boundary where no human reads. External references keep
  their textual paths (one string per item, structure only for
  applying arguments); boundary items that the outside world names
  (exports, external-trait impls) keep their required names.
  Diagnostics get un-mangled for display through the same span map
  that routes squiggles.
- **Schema by tiers**, with syn (~205 node kinds, `Expr` alone 40
  variants) as the measured ceiling: ~28 kinds writes the first
  plugin end to end, ~50 is comfortable authoring, and a **verbatim
  node** (syn ships the same escape hatch) holds anything not yet
  modeled — the schema's floor is one verbatim node holding a whole
  file, and every modeled kind is a strict improvement over it.
  Generic *application* is tier 0 (unwritable Rust without it);
  generic *definitions* with simple bounds are a later tier; `use`
  is not authored structure at all — emission synthesizes imports.
- **No importer required**: we author, we don't ingest. The editor's
  own support machinery stays ordinary text Rust — hosting raw.rs in
  the editor would prove a point, not serve one.

## The Spike (2026-07-25)

Landed: `compile.rs` (the pipe pipeline, with structured
diagnostics), `plugins.rs` (wasmtime host, watchdog, the call
protocol, and the f64 dispatch rule: a record whose single field is
the string label `"f64"` holding an eight-byte little-endian blob),
and `plugins/f64.rs` — the first plugin, checked in as ordinary text
Rust, compiled at launch if stale and cached under `target/plugins/`.
The raw projection substitutes the plugin's text for the record's
form outside the Raw view; the Raw toggle always shows structure as
stored. The sample document's roof gained a `"pitch"` so the
substitution is visible on open.
