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
Traditional:   Text → Parse → AST → (work) → Pretty-print → Text
Projectional:  Structure → Project → Display → Edit → Structure
```

The projection can be anything: tree view, text-like syntax, visual blocks, tables. The same structure could render as `a + b` or `Add(left: a, right: b)` or a node-and-wire diagram.

A key capability is **dual projection**: edit in a tree view (natural for hierarchical navigation) while visualizing in a bubbles-and-arrows graph view (shows true topology, cycles, multiple paths to the same node). Synchronized selection between views lets you work in whichever is most natural for the task.

Note: text files are also projected—syntax highlighting, fonts, line wrapping, unicode rendering. You're never seeing "raw bytes." Progred just projects from a higher level of abstraction.

This means code style (tabs/spaces, brace placement) becomes a projection preference, not a property of the data. No more flame wars—just different views of the same structure.

### 3. Semantic-Free Core

The core data model is just a labeled directed graph:

```
ID → label → ID
```

Where `ID` can be:
- **GUID**: Random string for mutable nodes
- **SID**: String literal (e.g., `"hello"`)—the string IS its own identity
- **NID**: Number literal—the number IS its own identity

The graph knows nothing about programming, types, functions, or scope. It's a dumb substrate that can represent anything—just like text is characters that can represent anything.

Semantics come from conventions built on top: certain identities are recognized as meaning "name" or "isa" or "fields." These conventions can vary, coexist, and evolve.

## Semantic Islands

Rather than a fixed type system, Progred supports **semantic islands**—collections of conventions that tools and renderers recognize.

Examples:
- A very broad convention: the `name` field (useful for almost anything)
- A type system inspired by Haskell ADTs (useful for programming)
- A specific language's AST conventions
- A recipe database schema
- A personnel records format

These aren't layers—they're just identities that various parts of the system treat specially. They can coexist in the same document. Different tools can recognize different subsets.

The "standard" semantic island (with `name`, `isa`, `fields`, type definitions, etc.) is just a library that can be auto-loaded, registering its identities with the editor. Or not—you can start raw.

## Bootstrapping

The editor should support two modes:

**With pre-loaded semantics**: Editor starts with standard identities (`name`, `isa`, etc.) already registered. Familiar starting point, can build immediately.

**Raw bootstrap**: Everything starts as identicons (visual hashes of GUIDs). Commands let you designate identities: "this node is now the name-field identity." Build up semantics from nothing, within the editor.

This enables:
- Demonstrating the system's foundations
- Creating alternative semantic islands
- True self-description—the editor's own conventions are editable data

## Rendering Progression

Without any recognized conventions:
- Nodes display as identicons (visual hash of GUID)
- Edges display with identicon labels
- Pure structure, no semantics assumed

With `name` convention:
- Nodes with a `name` edge display that name instead of identicon
- Otherwise still identicons

With `isa` convention:
- Nodes with an `isa` edge can use type-aware rendering
- Editor knows what fields to expect based on the type definition

With custom renderers:
- Types can have associated render definitions
- Full projectional flexibility

You can always request a "raw" view—the default spanning-tree render—to see what's actually there.

## Architecture

### Tech Stack
- **UI**: egui (immediate mode GUI)
- **Data structures**: Persistent immutable data structures (`im` crate)
- **TypeScript runtime**: Deno (embedded V8) for running TypeScript code and type checking
- **Build**: Cargo
- **Serialization**: JSON for now, binary formats later when tooling supports it

The core graph operations and UI live in Rust. We use the `im` crate for persistent immutable data structures—structural sharing means efficient cloning for undo/redo and snapshots. Deno's embedded V8 runtime lets us execute TypeScript directly and use the TypeScript compiler for type information.

### Core Data Model

```rust
// Identity types
enum Id {
    Guid(GuidId),      // Random identity for mutable nodes
    String(StringId),  // String literal as identity
    Number(NumberId),  // Number literal as identity
}

// The entire data model
type Graph = HashMap<GuidId, HashMap<GuidId, Id>>  // parent → label → child
```

Implementation in `src/id.rs` and `src/mutgid.rs`.

## Target Use Cases

gid can represent any structure, but we're starting with two:

**Code**: The obvious one. Programming benefits from structural editing—no syntax errors, semantic refactoring, flexible projections of the same logic.

**CAD/CAM**: Current tools like Fusion work with complex monolithic constructs for geometry and toolpath programming. We want an FP-inspired approach—composing simple abstractions to describe geometries and machine toolpaths. Same philosophy as code: simple primitives, composition, multiple projections (visual, textual, tabular).

Both domains benefit from gid's core properties: strong identity, projectional flexibility, semantic-free substrate.

## Pragmatic Choices

Some components are necessary but not where the innovation lies:

**Type system**: Continuing with a simplified Haskell-inspired ADT system. Proven approach, no fresh ideas needed here.

**Rendering language**: A minimal HTML-like structure for projections. Will evolve as needed, drawing on existing work rather than inventing something new.

**Programming language**: Using TypeScript rather than inventing a new language. This avoids a classic projectional editor mistake—inventing a language doubles your impossible problems. We can use Deno to run TypeScript code directly and leverage the TypeScript compiler for type checking and analysis. The innovation space is in *projections* of TypeScript (or other languages), not in the language itself.

## Current Status

Core data structures (ID types, MutGid graph, spanning tree traversal) are ported to Rust. Basic egui UI with tree view rendering. TypeScript runtime integration is in place for executing embedded code.

Next steps: cursor model, basic editing operations, identicon rendering, and projection system.
