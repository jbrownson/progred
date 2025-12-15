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

Note: text files are also projected—syntax highlighting, fonts, line wrapping, unicode rendering. You're never seeing "raw bytes." Progred just projects from a higher level of abstraction.

This means code style (tabs/spaces, brace placement) becomes a projection preference, not a property of the data. No more flame wars—just different views of the same structure.

### 3. Semantic-Free Core

The core data model is just a labeled directed graph:

```
ID → label → ID
```

Where `ID` can be:
- **GUID**: Random string for mutable nodes
- **SID**: String literal (e.g., `"sid:hello"`)—the string IS its own identity
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

V1 had a bootstrap problem: the type system was hardcoded in TypeScript because the editor couldn't create foundational types from scratch.

V2 should support two modes:

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

## Architecture (v2)

### Tech Stack
- **Frontend**: Solid.js + TypeScript
- **Desktop**: Tauri 2.0 (Rust)
- **Build**: Vite
- **Serialization**: JSON for now, binary formats later when tooling supports it

Most logic lives in TypeScript—Solid's signals for reactivity, TypeScript's compiler API for language integration. Tauri's Rust backend is there if we hit performance walls, but crossing the FFI boundary isn't worth it until then.

### Core Data Model

```typescript
type GUID = string        // Random identity for mutable nodes
type SID = string         // "sid:..." - string literal as identity
type NID = number         // Number literal as identity
type ID = GUID | SID | NID

// The entire data model
type Graph = Map<GUID, Map<ID, ID>>  // parent → label → child
```

### Minimal Kernel

The editor needs only:
1. **Raw graph ops**: create node, create edge, delete node, delete edge
2. **Navigation**: cursor model for tracking position in the graph
3. **Identicon rendering**: visualize any GUID without semantics
4. **Configurable special identities**: register which GUIDs mean `name`, `isa`, etc.
5. **Conditional rendering**: use recognized conventions when present, fall back to raw

Everything else—type system, custom renderers, libraries—builds on top.

## Development

```bash
# Install dependencies
npm install

# Run development server
npm run tauri dev

# Build for production
npm run tauri build
```

## Target Use Cases

gid can represent any structure, but we're starting with two:

**Code**: The obvious one. Programming benefits from structural editing—no syntax errors, semantic refactoring, flexible projections of the same logic.

**CAD/CAM**: Current tools like Fusion work with complex monolithic constructs for geometry and toolpath programming. We want an FP-inspired approach—composing simple abstractions to describe geometries and machine toolpaths. Same philosophy as code: simple primitives, composition, multiple projections (visual, textual, tabular).

Both domains benefit from gid's core properties: strong identity, projectional flexibility, semantic-free substrate.

## Pragmatic Choices

Some components are necessary but not where the innovation lies:

**Type system**: Continuing with a simplified Haskell-inspired ADT system. Proven approach, no fresh ideas needed here.

**Rendering language**: A minimal HTML-like structure for projections. Will evolve as needed, drawing on existing work rather than inventing something new.

**Programming language**: Using TypeScript rather than inventing a new language. This avoids a classic projectional editor mistake—inventing a language doubles your impossible problems. TypeScript's compiler API lets us feed it an AST directly from the graph, bypassing text entirely. We get type checking, red squiggles, and IDE features for free. The innovation space is in *projections* of TypeScript (or other languages), not in the language itself.

## Relationship to v1

V1 (in parent directory) is a working proof-of-concept using older tech (React, Electron, TypeScript 2.x). The core ideas are sound; v2 is a fresh implementation with:
- Modern tooling (Tauri, Solid.js, TypeScript 5.x)
- True bootstrap capability (create semantics from within)
- Cleaner separation of core vs conventions
- Lessons learned from v1's architecture
