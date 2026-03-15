# Structured Editor Type System Reference

## Status

This document describes the current design of the schema and type system for the structured editor project.

It is intended to be a **self-contained reference** for the model we converged on in design discussion. It records the core ideas, the current pseudo-syntax, and the intended semantics.

The design is intentionally small. It is not trying to be a general-purpose programming language type system. The goal is to define and validate structured editor data, including the schema language itself, inside one uniform graph.

---

## 1. Storage model

The editor's data model is a single graph:

```txt
Graph = Map<Id, Map<Id, Id>>
```

Where:

- `Id` may be a UUID-like identifier or a primitive identifier such as a number or string.
- **Every `Id` denotes a node in the universe of nodes.** There are no inherent leaves at the storage level.
- Some nodes may have no outgoing edges.
- Nodes are not created or deleted at the semantic level; the system mutates edges.
- Primitive ids and UUID-backed ids both live in the same universe.

This means the storage layer is extremely uniform:

- a schema declaration is just a node,
- a value node is just a node,
- a type expression is just a node,
- a primitive such as `42` is also just a node.

The distinctions between those are **semantic roles**, not structural categories in the graph.

---

## 2. Design goals

The current design aims to satisfy the following:

1. **Self-description**: the schema language should describe itself using the same graph model.
2. **Uniform representation**: schema declarations, type expressions, and user data all live in the same graph.
3. **Tagged structure**: value nodes have stable constructor/schema identity via a `record` edge.
4. **Contextual typing**: full type matching is contextual and may depend on type arguments supplied by surrounding context.
5. **First-order generics**: generic types such as `List<T>` should work without needing higher-kinded types.
6. **No positional type arguments**: type application should use parameter identity, not argument order.

Non-goals for the current design:

- higher-kinded types,
- type inference in the style of a programming language compiler,
- a fully structural, tag-free type system,
- forcing every value node to carry a complete intrinsic type independent of context.

---

## 3. Semantic roles

The graph itself does not separate nodes into categories. The following words describe **how a node is being used**, not what kind of thing it inherently is:

- **value node**: a node being checked as editor data,
- **record declaration**: a node that describes a constructor/schema head,
- **sum declaration**: a node that describes a choice among type expressions,
- **type expression**: a node used inside typing/matching,
- **field declaration**: a node that names a field and gives its expected type expression,
- **type parameter**: a node used as a binder/placeholder in type expressions.

A node may be part of the metamodel and also be edited as ordinary data, because the schema defines itself.

---

## 4. Core semantic distinction

Two judgments are deliberately kept separate:

### 4.1 `recordOf(v)`

For a value node `v`, `recordOf(v)` is read from its `record` edge.

This tells us the node's **constructor/schema head**.

Example:

```txt
recordOf(v) = Cons
```

This means the node is a `Cons` instance. It does **not** tell us whether the node is being used as `List<Number>`, `List<String>`, or something else.

### 4.2 `matches(v, T, σ)`

`matches(v, T, σ)` means value `v` matches type expression `T` under substitution environment `σ`.

This is contextual. It depends on:

- the expected type expression,
- the current type-parameter substitution,
- the schema reachable from the graph.

So `record` is not the full type. It is only the head used to inspect fields and identify the node's shape.

---

## 5. Pseudo-syntax conventions

The text notation in this document is only a readable projection of graph nodes.

Conventions:

- Human-readable names stand in for UUIDs.
- All non-primitive nodes still have their own ids in the real graph.
- Inline declarations in the pseudo-syntax are not structurally different from top-level declarations. Inline presentation is only notation.

### 5.1 Declaration sugar

These are surface forms used for readability:

```txt
Field "x" { ... }
Record "Foo" { ... }
Sum "Bar" { ... }
```

Each of these stands for a node whose own `record` edge points to `Field`, `Record`, or `Sum` respectively.

### 5.2 Type application sugar

If `F` is a `Record` or `Sum` declaration with type parameters:

```txt
[P1, ..., Pn]
```

then:

```txt
F<A1, ..., An>
```

is shorthand for:

```txt
Apply {
    type function: F,
    P1: A1,
    ...
    Pn: An
}
```

Important:

- the labels `P1 ... Pn` are the actual **Type Parameter node ids**, not strings,
- arguments are assigned by **parameter identity**, not position,
- `Apply` exists because declarations are shared and therefore cannot store per-use type assignments directly on themselves.

### 5.3 Nested declarations and scope

Nested declarations may refer to type parameters bound by enclosing `Record` or `Sum` declarations.

That means a nested declaration may be **open** in isolation and only become meaningful in the environment provided by its enclosing declaration.

This is intentional.

---

## 6. Core schema definition

The current core schema is:

```txt
# Notes:
# - names stand in for uuids in this pseudo-syntax
# - if F is a Record or Sum with type parameters [P1, ..., Pn], then:
#       F<A1, ..., An>
#   is shorthand for:
#       Apply {
#           type function: F,
#           P1: A1,
#           ...
#           Pn: An
#       }
# - the Pi labels above are the actual Type Parameter node ids
# - nested declarations may refer to type parameters bound by enclosing Record/Sum declarations

Field "name" {
    type expression: String
}

Field "record" {
    type expression: Record
}

Field "type expression" {
    type expression: Type Expression
}

Field "type parameters" {
    type expression: List<Type Parameter>
}

Field "type function" {
    type expression: Type Function
}

Record "String" {
    type parameters: [],
    fields: []
}

Record "Number" {
    type parameters: [],
    fields: []
}

Record "Type Parameter" {
    type parameters: [],
    fields: [
        name
    ]
}

Record "Field" {
    type parameters: [],
    fields: [
        name,
        type expression
    ]
}

Record "Record" {
    type parameters: [],
    fields: [
        name,
        type parameters,
        Field "fields" {
            type expression: List<Field>
        }
    ]
}

Record "Sum" {
    type parameters: [],
    fields: [
        name,
        type parameters,
        Field "summands" {
            type expression: List<Type Expression>
        }
    ]
}

Record "Apply" {
    type parameters: [],
    fields: [
        type function
    ]
}

Sum "Type Function" {
    type parameters: [],
    summands: [
        Record,
        Sum
    ]
}

Sum "Type Expression" {
    type parameters: [],
    summands: [
        Record,
        Sum,
        Apply,
        Type Parameter
    ]
}

Sum "List" {
    type parameters: [
        Type Parameter "T" {}
    ],
    summands: [
        Record "Cons" {
            type parameters: [],
            fields: [
                Field "head" {
                    type expression: T
                },
                Field "tail" {
                    type expression: List<T>
                }
            ]
        },
        Record "Empty" {
            type parameters: [],
            fields: []
        }
    ]
}
```

---

## 7. What each construct means

### 7.1 `Record`

A `Record` declaration describes a constructor/schema head.

A value node with:

```txt
record: R
```

is saying that it is an instance of record declaration `R`.

A `Record` declaration also carries:

- a human-readable `name`,
- a list of bound `type parameters`,
- a list of declared `fields`.

### 7.2 `Sum`

A `Sum` declaration describes a choice among type expressions.

A value matches a `Sum` when it matches one of the sum's instantiated summands.

A `Sum` declaration also carries:

- a `name`,
- a list of bound `type parameters`,
- a list of `summands`.

### 7.3 `Type Parameter`

A `Type Parameter` is the only primitive open form.

Anything else is open only because it contains one or more type parameters that are not yet instantiated.

### 7.4 `Apply`

`Apply` is the node that assigns actual type arguments to a parameterized `Record` or `Sum`.

This is represented by:

- a required `type function` edge,
- one edge per bound type parameter,
- where the **edge labels are the parameter nodes themselves**.

So if `List` binds parameter `T`, then:

```txt
List<Number>
```

is represented as:

```txt
Apply {
    type function: List,
    T: Number
}
```

This avoids order-based assignment and makes application naturally map-shaped.

### 7.5 `Type Function`

`Type Function` is currently:

```txt
Record | Sum
```

This means:

- first-order generic records and sums are supported,
- higher-kinded type parameters are not part of the current design.

---

## 8. Open, closed, and well-scoped expressions

The system needs a scoping notion because nested declarations may refer to type parameters from outer declarations.

### 8.1 Open expression

A type expression or declaration is **open** if it contains one or more free `Type Parameter` references.

### 8.2 Closed expression

A type expression or declaration is **closed** if it contains no free type parameters.

### 8.3 Well-scoped expression

An expression or declaration is **well-scoped** relative to an environment `Γ` if every free type parameter it contains is a member of `Γ`.

Informally:

- `Type Parameter "T"` is well-scoped only when `T` is bound in the current environment,
- a `Record` or `Sum` extends the environment with its own `type parameters` when checking its interior,
- nested declarations may capture the outer environment.

In the current model, this is acceptable and intentional.

---

## 9. Resolved vs unresolved type expressions

Not every node that matches `Type Expression` is immediately usable as a fully matchable type.

It is useful to distinguish:

### 9.1 Type expression

Anything described by `Type Expression`, including:

- `Record`,
- `Sum`,
- `Apply`,
- `Type Parameter`.

### 9.2 Resolved type expression

A type expression is **resolved** when it has enough information to be matched against values in the current environment.

Examples:

- `String` is resolved.
- `Number` is resolved.
- `List<Number>` is resolved.
- `T` is resolved only if the current substitution binds `T`.
- bare `List` is not resolved as a final value type, because it is a type function awaiting arguments.

This distinction lets the schema be self-describing without requiring every type-expression node to already be fully instantiated.

---

## 10. Matching semantics

The central semantic judgment is:

```txt
matches(value, type expression, substitution)
```

Where:

- `value` is a graph node used as editor data,
- `type expression` is the expected type,
- `substitution` maps type parameter nodes to type expressions.

### 10.1 Matching records

A value `v` matches record declaration `R` under substitution `σ` when:

1. `R` is interpreted using its bound type parameters and the current substitution,
2. `v.record = R`,
3. for each declared field in `R.fields`, the target of that field matches the field's `type expression` after substitution.

The `record` edge identifies the node's head shape; the field types still depend on context.

### 10.2 Matching sums

A value `v` matches sum declaration `S` under substitution `σ` when:

1. `S` is interpreted under `σ`, and
2. `v` matches at least one instantiated summand of `S`.

### 10.3 Matching type parameters

A value `v` matches type parameter `T` under substitution `σ` when:

- `σ(T)` exists, and
- `v` matches the substituted type expression `σ(T)`.

### 10.4 Matching applications

A value `v` matches `Apply { type function: F, P1: A1, ..., Pn: An }` under substitution `σ` by:

1. reading the bound type parameters `[P1, ..., Pn]` from `F`,
2. extending the substitution with:

   ```txt
   P1 ↦ A1
   ...
   Pn ↦ An
   ```

3. then matching `v` against the body of `F` under the extended substitution.

For a `Record`, the "body" is its field declarations.

For a `Sum`, the "body" is its summands.

### 10.5 Correctness vs representability

The raw graph can represent many incorrect states.

That is expected.

Examples of incorrect-but-representable states:

- a malformed `Apply` missing one required parameter edge,
- an `Apply` with an unexpected extra parameter edge,
- a value node whose `record` points to a `Record` but whose fields do not match,
- a UUID-backed node that claims to be `Number` in a way the checker does not accept.

The graph is like source text: it can represent incorrect programs. The type system determines correctness; it does not restrict what the graph can store.

---

## 11. Primitive ids

All ids are nodes in the same universe, including numbers and strings.

The current design leaves room for primitive ids to expose procedural or derived edges such as:

- `record -> Number` or `record -> String`,
- `name -> "42"`,
- other inspection metadata.

This is compatible with the uniform graph model.

However, matching `Number` and `String` may still be defined semantically by primitive-id kind rather than only by stored edges, if that proves more robust in practice.

The important point is:

- primitive ids are not storage-level leaves,
- but the checker may still give them special matching rules.

---

## 12. Example: `List<T>`

The reference example is:

```txt
Sum "List" {
    type parameters: [
        Type Parameter "T" {}
    ],
    summands: [
        Record "Cons" {
            type parameters: [],
            fields: [
                Field "head" {
                    type expression: T
                },
                Field "tail" {
                    type expression: List<T>
                }
            ]
        },
        Record "Empty" {
            type parameters: [],
            fields: []
        }
    ]
}
```

### 12.1 `List<Number>`

The type expression:

```txt
List<Number>
```

means:

```txt
Apply {
    type function: List,
    T: Number
}
```

### 12.2 Matching a `Cons`

Suppose a value node looks like:

```txt
{
    record: Cons,
    head: 1,
    tail: ...
}
```

This node does **not** intrinsically tell us whether it is a `List<Number>` or a `List<String>`.

It only tells us:

```txt
recordOf(v) = Cons
```

When we match it against `List<Number>`, the surrounding `Apply` provides:

```txt
T ↦ Number
```

So the `Cons` fields are interpreted as:

- `head : Number`
- `tail : List<Number>`

The same `Cons` head could participate in a different instantiated sum in different contexts.

### 12.3 Matching `Empty`

Similarly, a node with:

```txt
record: Empty
```

may match many instantiated list types depending on context.

This is expected. Empty constructors are often ambiguous without surrounding type information.

---

## 13. Why `Apply` exists

An obvious idea is to try to store type-argument assignments directly on the `Record` or `Sum` being instantiated.

That does not work well because declarations are shared. The same declaration may be used simultaneously at different instantiations:

- `List<Number>`
- `List<String>`
- `List<Expr>`

So the type-argument assignment needs its own node.

`Apply` is that node.

It is the type-level analogue of a value instance:

- a value instance says which `record` it has,
- a type application says which `type function` it instantiates and with which parameter assignments.

---

## 14. Why values do not repeat type arguments by default

One possible extension would be to let value nodes repeat type-argument assignments, in the same way that they carry a `record` edge.

For example, a `Cons` node might carry something like:

```txt
T: Number
```

in addition to being checked under `List<Number>`.

The current design does **not** include this.

Reason:

- it duplicates contextual information,
- it adds many extra incorrect states,
- it is not needed for matching,
- it becomes especially awkward for recursive structures such as lists.

Instead, the current model keeps value nodes lightweight:

- value nodes carry `record`,
- full type instantiation comes from context via `Apply`.

An explicit type annotation node may still be added later if the editor needs to pin a root value to a particular instantiated type.

---

## 15. Bootstrapping and self-description

The schema defines itself.

This means the reference definitions above are intentionally cyclic.

For example:

- `Field.type expression` refers to `Type Expression`,
- `Type Expression` includes `Record`, `Sum`, and `Apply`,
- `Record` and `Sum` both use `List<...>` in their own field types,
- `List` itself is defined using the same schema constructs.

This is not a problem in the graph representation. The graph is not evaluated top-to-bottom like a text file. All nodes coexist and refer to each other by id.

The ordering in this document is only a presentation order.

---

## 16. Kernel conventions

The design relies on a small number of kernel-level conventions that are not themselves expressed purely as ordinary schema data.

The important ones are:

1. **Value head convention**
   - A value node's meaningful schema head is determined by its `record` edge.

2. **Type application convention**
   - An `Apply` node's meaningful argument edges are determined by the `type parameters` of its `type function` target.

These are acceptable departures from total self-encoding because the system already needs a kernel rule for `record` on value instances.

---

## 17. Intentionally undecided policy points

The current design does not force answers to all editor/checker policy questions.

These remain open:

### 17.1 Exact vs open records

The graph can contain extra fields beyond those declared by a `Record`.

The checker/editor may choose either of these policies:

- **strict**: extra fields make the node incorrect,
- **permissive**: extra fields are tolerated but may be ignored or warned about.

The core schema does not yet encode this choice.

### 17.2 Primitive matching details

Primitive ids may expose derived edges such as `record` and `name`, but the exact matching rule for `String` and `Number` is still an implementation detail.

### 17.3 Type annotations on values

The current core does not require values to store explicit instantiated types. Such annotations may be added later as an editor aid without changing the main type-expression model.

---

## 18. Summary

The current design can be summarized as follows:

- The editor stores one uniform graph: `Map<Id, Map<Id, Id>>`.
- All ids, including primitive ids, are nodes in that universe.
- The schema language defines itself in the same graph.
- Value nodes carry a `record` edge identifying their constructor/schema head.
- Full type matching is contextual and uses `matches(value, type expression, substitution)`.
- `Record` and `Sum` bind their own `type parameters`.
- `Apply` instantiates a parameterized `Record` or `Sum`.
- `Apply` assigns arguments by **parameter identity**, using the `Type Parameter` nodes themselves as edge labels.
- `Type Parameter` is the only primitive open form.
- Nested declarations may refer to type parameters bound by enclosing declarations.
- The graph may represent incorrect states; correctness is determined by the type system, not enforced by storage.

This gives the project:

- tagged, inspectable structure,
- contextual generic typing,
- self-description,
- a graph-native representation of type application,
- and a relatively small core.
