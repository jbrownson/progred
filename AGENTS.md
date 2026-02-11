# Development Notes

## Cargo Build Cache

Cursor's sandbox sets `CARGO_HOME` and `RUSTUP_HOME` to temporary directories. This causes cache invalidation when alternating between terminal and Cursor builds.

When running cargo commands, unset these:

```bash
unset CARGO_HOME RUSTUP_HOME && cargo build
```

## Workflow

- Don't run the app — the user prefers to run and test it themselves
- Don't fight the system — avoid hacks/workarounds that go against how frameworks or platforms are designed; push back early when something seems like it's not meant to work that way
- Keep unrelated changes in separate commits whenever possible; avoid bundling housekeeping with feature work
- Only commit when explicitly asked

## Git Commits

zsh heredocs fail in the sandbox because zsh writes temp files to `$TMPPREFIX` (defaults to `/tmp/zsh`), which is outside the sandbox allowlist. Fix by exporting it before the commit:

```bash
export TMPPREFIX=/tmp/claude/zsh && git commit -m "$(cat <<'EOF'
Commit message here

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
EOF
)"
```

## Key Design Rules

- **Documents are pure graph structure** — No semantic interpretation baked in. Use generated constants (`Field::NAME`, `Field::ISA`, etc.) for semantic access.

## Future Considerations

- `muda` crate for native OS menus (instead of egui menus)

## TODOs

- **Domain-specific projections**: Make them more editable (currently mostly read-only)
- **Autocomplete integration**: Hook up name lookups to the placeholder autocomplete dialog, port architecture from original prototype
- **Red squiggles**: Real-time type system errors displayed inline
- **Default projection improvements**: Show placeholders for missing fields, order fields per record definition, show extra fields at bottom

## Code Style

- Very limited comments — code should be self-documenting
- Expression-oriented where possible
- Prefer long expressions broken across multiple lines over multiple statements with intermediate names — naming is hard, avoid unnecessary names
- Exception: extract helper functions when intermediate steps represent distinct semantic concepts — top-level function becomes a readable composition of named transformations (functional decomposition)
- Prefer free functions with explicit parameters over methods when `self` isn't needed — makes inputs/outputs clear, easier to unit test, enables composition in a single method that has access to `self` (Haskell-style)
- Look for generic abstractions — extract patterns in how computations combine and data flows (the way `fold`/`map`/monads abstract over structure, not specific operations)
- Apply Haskell-style thinking (explicit data flow, pure function composition) but idiomatic Rust syntax — don't fight the language
- Factor out common assignments: `x = if cond { a } else { b }` not `if cond { x = a } else { x = b }`
- Functional style: iterator chains, `try_fold`, `filter_map`, `std::array::from_fn` over mutable accumulators and loops where it doesn't make things worse
- Avoid `let mut` when a functional alternative is equally clear
- No `isX()` predicate methods — use `matches!` or pattern matching at the call site
- Eliminate partial functions: use `split_first`/`split_last` over manual indexing
- Return references for non-trivial types (let caller decide to clone), but methods on small structs (like `Document`) enable disjoint borrow checking over methods on the parent (`ProgredApp`)
- Avoid early returns — prefer `if let`, `match`, or expression-oriented alternatives over `let-else return` / `return` in closures
- Dead code should be deleted, not commented out
- UI rendering: prefer computing from an immutable snapshot and mutating via a writer (e.g., `EditorWriter`) over returning update structs — not a hard rule, deviate if there's reason
- Push back if something seems wrong
