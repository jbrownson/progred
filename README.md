# Progred

Progred is a projectional editor aimed at Inventing-on-Principle-style
CAD/CAM: the editable source is structured data, and projections can present
that data as code, controls, diagrams, or live geometry without translating it
through a canonical source text.

The project is now a working native and browser prototype. It includes a small
graph language, Grap; a projection stack; a continuation-based layout and event
pipeline; and Puri backends for Vello and Canvas2D.

## The stack

- **GID** is the native data substrate: cell references, blobs, lists, and
  records. Cell identity is separate from every naming convention. See
  [`docs/gid.md`](docs/gid.md).
- **Libraries** add open conventions such as names, UTF-8 text, numbers,
  colors, geometry, and layout. None of these are GID primitives.
- **Grap** is a small graph-processing language embedded directly in GID. Its
  functions consume and produce GID values, while Rust functions supply the
  primitive operations and platform capabilities.
- **Projections** are composable partial functions with a raw projection at
  the root. A projection may redispatch children through the same stack, add a
  contextual projection, evaluate Grap, or produce a domain-specific view.
- **Display and layout** describe editable vector-graphics leaves and
  pretty-printer-like structure. Placement settles geometry before hover and
  rendering continuations run.
- **Puri** is the pure widget boundary. The normal renderer calls Vello or
  Canvas2D directly; recorders can reify the same final-tagless drawing
  language for tests and debugging. See [`docs/puri.md`](docs/puri.md).

The current model and projection implementation are described in
[`docs/model.md`](docs/model.md) and [`docs/projections.md`](docs/projections.md).
The broader motivation is in [`MOTIVATION.md`](MOTIVATION.md).
See the [documentation index](docs/README.md) for current references, deferred
work, and separately archived historical notes.

## Run it

On macOS, use the repository's sandboxed build and runtime workflow:

```sh
make run
```

This is the development replacement for `cargo run --release`. It builds in an
isolated directory, packages an ad-hoc-signed App Sandbox bundle, and launches
that bundle. Other useful commands are:

```sh
make sandbox-check
make sandbox-test
make serve-web
make build-ipad
```

For an interactive native development session, use `make dev`: Ctrl+C rebuilds
and restarts its app, and Ctrl+\ quits. A failed build waits for another Ctrl+C.

The browser build is served at `http://localhost:8080`; another device on the
same network can use this machine's LAN address. Built-in documents are
available from the Examples menu. The native iPad host and deferred spatial
visionOS work are documented in [`docs/platforms.md`](docs/platforms.md).

Cargo build scripts and procedural macros execute dependency code. Ordinary
Cargo commands in this repository intentionally stop at a tripwire; read
[`docs/build-security.md`](docs/build-security.md) before changing dependencies
or bypassing the supplied commands.

Before distributing a build, review [`docs/release-checklist.md`](docs/release-checklist.md).

Sample documents under `examples/` include:

- `examples/sample.gid` — editor and projection examples;
- `examples/grap-demo.gid` — Grap language constructs;
- `examples/iop-tree.gid` — the first tree from Bret Victor's *Inventing on Principle*,
  recreated as an editable Grap program and live drawing.

The checked-in GID notation is a temporary bridge for Git, debugging, and
text-bound tooling. GID is intended to become its own non-textual binary stack;
the bridge is documented in [`docs/gid-text.md`](docs/gid-text.md).

## Repository layout

- `gid/` — GID values, cells, documents, and positions
- `grap/` — Grap evaluator
- `libraries/` — core Progred libraries and their vocabulary/FFIs
- `display/` — display language
- `ui/` — measurement, layout, Puri, and native/web drawing backends
- `progred/` — editor application and projections
- `docs/` — current design and implementation notes
- `examples/` — checked-in GID example documents
- `experiments/` — focused research retained alongside the main project
- `reference/` — source material used to reproduce external examples

## Prototype history

The former TypeScript, Swift, egui, Haskell, and nested Linebender prototypes
were archived when this implementation took over the repository root. Their
complete pre-promotion source tree is available at the remote tag
`archive/pre-root-promotion`; `archive/multi-prototype` retains an earlier
multi-prototype milestone.

To inspect the repository immediately before promotion without disturbing the
current checkout:

```sh
git worktree add ../progred-before-promotion archive/pre-root-promotion
```

The most relevant design handoffs were retained in
[`docs/history/`](docs/history/).
