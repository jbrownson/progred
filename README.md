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

gid separates these: a node's identity is its UUID. Its name is just data attached to that identity—an edge labeled `name` pointing to a string. You can:
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
- **UUID**: Opaque identity for mutable nodes
- **String**: String literal—the string IS its own identity
- **Number**: Number literal—the number IS its own identity

The graph knows nothing about programming, types, functions, or scope. It's a dumb substrate that can represent anything—just like text is characters that can represent anything.

Semantics come from conventions built on top: certain identities are recognized as meaning "name" or "record" or "fields." These conventions can vary, coexist, and evolve.

## Target Use Cases

gid can represent any structure, but we're starting with two domains:

**Code**: Programming benefits from structural editing—no syntax errors, semantic refactoring, flexible projections of the same logic.

**CAD/CAM**: Current tools like Fusion work with complex monolithic constructs for geometry and toolpath programming. We want an FP-inspired approach—composing simple abstractions to describe geometries and machine toolpaths. Same philosophy as code: simple primitives, composition, multiple projections (visual, textual, tabular).

## Architecture

### Storage Model

```swift
// Identity types
enum Id {
    case uuid(UUID)
    case string(String)
    case number(Double)
}

// The entire graph: entity -> label -> value
// Map<UUID, Map<Id, Id>>
```

Single-valued edges: each entity has at most one value per label. Multiplicity is explicit via List (cons/empty linked list in the graph).

### Type System

A self-describing schema defined in the graph itself. See `progred/structured-editor-type-system-reference.md` for the full design. Key constructs:

- **Record**: constructor/schema head with named fields
- **Sum**: choice among type expressions
- **Apply**: instantiates a generic Record or Sum, using type parameter node IDs as edge labels (no positional arguments)
- **Field**: names a field and gives its expected type expression
- **Type Parameter**: binder/placeholder in type expressions

Value nodes carry a `record` edge identifying their constructor. Full type matching is contextual via `matches(value, type expression, substitution)`.

### Tech Stack

- **Language**: Swift
- **UI**: AppKit (macOS)
- **Data structures**: `TreeDictionary`/`TreeSet` from swift-collections (CHAMP-based persistent collections with structural sharing)
- **JS runtime**: JavaScriptCore (for projection code)

### Previous Prototypes

- `prototype-ts/` — Original TypeScript/Electron prototype
- `prototype-rust/` — Rust/egui prototype (moved to Swift due to egui's limited control over focus, text editing, and input handling)
