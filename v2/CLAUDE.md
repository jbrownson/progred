# Code Style

Haskell-inspired, expression-oriented TypeScript.

## Expressions over statements
- Prefer ternary expressions (`?:`) over early returns unless there are many variable definitions that make it awkward
- In ternaries, prefer testing the positive case: `x ? A : B` not `!x ? B : A`
- Functions should return expressions where possible, not build up results with statements

## Types
- Use `Maybe<T>` (T | undefined) instead of null
- Use `matchId`-style pattern matching functions for sum types
- Type aliases for domain concepts: `type Path = GuidId[]`
- Classes for data only (immutable value objects), functions for behavior
- Readonly properties on classes

## Functional patterns
- Pure functions where possible
- Immutable data structures (using immutable.js Map)
- Standard combinators: `mapMaybe`, `flatMapMaybe`, `traverse`, `firstMap`
- Records over tuples for multi-value returns: `{ parent, label }` not `[parent, label]`

## Naming and documentation
- No comments - code should be self-documenting through clear naming
- Small, focused functions with descriptive names
- Prefer explicit function parameters over closures that capture ambient state

## Formatting
- Single-line functions for simple expressions: `function foo(): T { return expr }`
- Arrow functions for callbacks and inline functions
- Destructuring for unpacking objects and arrays

## Collaboration
- Push back if something seems wrong or the user may have missed something
- Don't agree too quickly with suggested changes without thinking them through
- Read README.md for project vision and goals
