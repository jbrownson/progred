# Progred

A structural editor that preserves what makes text great—simplicity, flexibility, universality—while lifting the abstraction level to represent actual structures.

## Core Philosophy

Most projectional/structural editors fail because they trade text's flexibility for structure's benefits. They build an editor for *a specific AST*—tightly coupling the tool to a particular domain.

Text survives because it's a **universal substrate**:
- **Simple**: Just a sequence of characters—easy to build tools for.
- **Universal**: That simplicity means tools and editors are everywhere.
- **No assumed structure**: Encodes anything without forcing a schema.

**gid** is a data model with the same properties, one notch higher in abstraction. **Progred** is an editor for gid, like vim/vscode are editors for text.

## Three Key Ideas

### 1. Strong Identity, Separate from Naming

In text, identity IS naming. To reference a function, you write its name. This conflates:
- **What something IS** (its identity)
- **How we REFER to it** (a name, icon, position—context-dependent)

This conflation causes problems: renaming requires find-and-replace, shadowing creates ambiguity, name resolution needs scope rules, two things can't share a name in the same scope.

gid separates these: a node's identity is its GUID. Its name is just data attached to that identity—an edge labeled `name` pointing to a string. You can:
- Rename freely without breaking references
- Have duplicate names, or no name at all
- Compute names, use icons, support multiple languages
- Show different names in different contexts

All without losing strong identity.

### 2. Projectional Editing

There is no "source text." The graph structure IS the source of truth. What you see on screen is a *projection*—one of many possible views.

```
Traditional:   Text -> Parse -> AST -> (work) -> Pretty-print -> Text
Projectional:  Structure -> Project -> Display -> Edit -> Structure
```

The projection can be anything: tree view, text-like syntax, visual blocks, tables. The same structure could render as `a + b` or `Add(left: a, right: b)` or a node-and-wire diagram.

A key capability is **dual projection**: edit in a tree view (natural for hierarchical navigation) while visualizing in a bubbles-and-arrows graph view (shows true topology, cycles, multiple paths to the same node). Synchronized selection between views lets you work in whichever is most natural for the task.

### 3. Semantic-Free Core

The core data model is just a labeled directed graph:

```
Id -> label -> Id
```

Where `Id` can be:
- **GUID**: Opaque identity for mutable nodes
- **SID**: String literal, encoded as `sid:...`; the string IS its own identity
- **NID**: Number literal; the number IS its own identity

The graph knows nothing about programming, types, functions, or scope. It's a dumb substrate that can represent anything—just like text is characters that can represent anything.

Semantics come from conventions built on top: certain identities are recognized as meaning "name" or "record" or "fields." These conventions can vary, coexist, and evolve.

## Target Use Cases

gid can represent any structure, but we're starting with two domains:

**Code**: Programming benefits from structural editing—no syntax errors, semantic refactoring, flexible projections of the same logic.

**CAD/CAM**: Current tools like Fusion work with complex monolithic constructs for geometry and toolpath programming. We want an FP-inspired approach—composing simple abstractions to describe geometries and machine toolpaths. Same philosophy as code: simple primitives, composition, multiple projections (visual, textual, tabular).

## Architecture

### Storage Model

```ts
type GUID = string
type SID = string
type NID = number
type ID = GUID | SID | NID

// Stored outgoing edges exist for GUID nodes.
// Primitive string/number IDs are eternal nodes with library-provided edges.
type GUIDMap = Map<GUID, Map<ID, ID>>
```

Single-valued edges: each entity has at most one value per label. Multiplicity is explicit via List (cons/empty linked list in the graph).

### Type System

The TypeScript prototype currently uses a self-describing graph schema with generated wrappers in `prototype-ts/src/graph/graph.ts`. The current schema language is the older `Ctor` / `AlgebraicType` model:

- **Ctor**: constructor/schema head for a node
- **Field**: named edge with an expected type
- **AlgebraicType**: choice among constructors or nested algebraic types
- **ListType**: expected list element type
- **AtomicType**: primitive string/number types

Value nodes carry a `ctor` edge (`ctorField`) identifying their constructor. The Swift reference in `prototype-swift/structured-editor-type-system-reference.md` describes a more general target direction with records, sums, type parameters, and `Apply`; it is not the exact TypeScript implementation today.

Full type matching remains contextual and intentionally tolerant of malformed graph states.

### Current Prototype

The active path is `prototype-linebender/`: Progred built on Puri, a
pure widget library where widgets are pure functions from (persistent
widget state, props) to (draw calls, handlers), over the Linebender
stack (winit, Vello, Parley, kurbo, peniko). Puri holds no state between
frames, mints no identity, and ships no general layout engine — document
layout is a small baseline-aware box algebra; state management is
deliberately out of scope. See `prototype-linebender/docs/puri.md` for
the UI runtime decision and plan, and `prototype-linebender/docs/model.md`
for the data and editor model decisions.

The Haskell/Wasm prototype (`prototype-haskell/`) is paused; it produced
the Puri and Halay designs the Rust direction carries forward (see
`prototype-haskell/RUST_PIVOT.md`). The previous `prototype-ts/`
TypeScript/Electron prototype is usable and remains the most complete
editor and the behavior reference for editing semantics, but React/DOM
focus and local component state repeatedly caused subtle synchronization
bugs around selection, secondary selection, collapse/layout state, and
pending editors.

Useful commands:

```sh
cd prototype-linebender
unset CARGO_HOME RUSTUP_HOME && cargo build
unset CARGO_HOME RUSTUP_HOME && cargo test

cd prototype-ts
npm install
npm start
npm test
npm run typecheck
npm run build
npm run build:fidget
npm run gen
npm run graph -- inspect src/graph/libraries/type.progred
npm run graph -- render src/graph/libraries/type.progred
```

`npm start` builds the app and launches Electron. `npm run gen` rebuilds the graph wrappers from the graph schema and rewrites `src/graph/graph.ts` and `src/graph/renderIfs.ts`; inspect the diff after running it.
`npm run build` runs `npm run build:fidget` first. The Fidget bridge requires Rust with the `wasm32-unknown-unknown` target and `wasm-bindgen` available on PATH.
`npm run graph -- ...` builds and runs a read-only graph CLI. Use `find` to search graph/library names, `inspect` to print structural edges and list contents, and `render` to server-render the actual editor projection as pretty-printed static markup.

### Other Prototypes

- `prototype-egui/` — Rust/egui shell and reusable graph/core crates (formerly `prototype-rust/`), historical; egui focus post-mortem in `prototype-egui/AGENTS.md`
- `prototype-haskell/` — Haskell/Wasm prototype using the Puri canvas runtime and the Halay layout engine, paused; see `prototype-haskell/RUST_PIVOT.md`
- `prototype-ts/` — TypeScript/Electron prototype using React DOM, paused; the most complete editing behavior and test suite
- `prototype-swift/` — Swift/AppKit native exploration
