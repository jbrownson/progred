# Development Notes

Read `README.md` first for project philosophy, active prototypes, and current status.

## Workflow

- Don't run GUI apps unless the user asks; they usually prefer to run and test interactive apps themselves.
- Don't fight the system. Avoid hacks/workarounds that go against how frameworks or platforms are designed; push back early when something seems like it is not meant to work that way.
- Treat compact or surprising policy couplings as intentional until proven otherwise. If a value is reused as a proxy for another concept, stop and name the concept, add a regression test, or ask before changing it.
- Intentional compromises are allowed, but make them conscious: use a short comment explaining the invariant or platform constraint, and prefer names like `mountedSingleLine` over unrelated booleans like `defaultCollapsed`.
- Keep unrelated changes in separate commits whenever possible.
- Only commit when explicitly asked.
- When a design pattern or lesson emerges during work, propose adding it to the relevant doc so future sessions start with that knowledge.

## Active TypeScript Prototype

The main editor prototype is `prototype-ts/`.

```bash
cd prototype-ts
npm install
npm start
npm test
npm run typecheck
npm run build
npm run build:fidget
npm run gen
```

`npm start` builds the app and launches Electron. `npm run gen` rebuilds generated graph wrappers from graph libraries; inspect the diff after running it.
`npm run build` regenerates the Fidget wasm bridge first; that path expects Rust with the `wasm32-unknown-unknown` target and `wasm-bindgen`.

The TypeScript prototype has a read-only graph CLI:

```bash
cd prototype-ts
npm run graph -- find src/graph/libraries/type.progred "Ctor"
npm run graph -- inspect src/graph/libraries/type.progred
npm run graph -- render src/graph/libraries/type.progred
```

Use `find` for named nodes/fields/ctors, `inspect` for structural edges and list contents, and `render` for the actual editor projection rendered to pretty-printed static markup. This is useful when reviewing or editing `.progred` graph libraries without reading raw JSON.

## Current TypeScript Architecture Notes

- Documents are pure graph structure. Semantics come from graph libraries and conventions.
- Mutable nodes are GUIDs. Strings are SIDs (`sid:...`). Numbers are NIDs.
- The React/DOM renderer has repeated synchronization problems around browser focus, selection, secondary selection, local collapse/layout state, and pending editors. Do not add more focus-sync workarounds casually; `graphEditor.integration.test.tsx` contains an expected-failing test for graph primary selection staying active after document focus returns.
- The current schema language uses `Ctor`, `Field`, `AlgebraicType`, `ListType`, and `AtomicType`; value nodes use the `ctorField` edge.
- DOM focus is the source of truth for the active editor target. Avoid reintroducing parallel selection state unless a real selection cannot be represented by focus.
- Editors attach commands/callbacks to their focused DOM elements. Prefer local projection-owned behavior over global cursor/path reconstruction.
- Lists intentionally have custom projection/editing behavior because their displayed insertion points do not match the linked-list graph shape.
- `D.singleLine` is parent layout metadata, not just a child implementation detail. Components with local state, especially `collapsible`, must describe the layout they mount with; if state changes can alter layout, call out the compromise explicitly.
- Rendering must tolerate malformed graph states: missing fields render as placeholders, unexpected values fall through to default/raw rendering, and extra fields remain visible.
- Library/source metadata controls read-only behavior. Do not bypass it when adding editing paths.

## Code Style

- Very limited comments. Code should be self-documenting; comment only when a short note prevents non-obvious misreadings.
- Prefer expression-oriented code and inline one-off intermediate values.
- Extract helpers when a step is semantically distinct, reused, or helps type inference.
- Prefer small functions over monolithic functions.
- Avoid partial functions and unsafe downcasts where the type can carry the distinction.
- Dead code should be deleted, not commented out.
- Push back if something seems wrong.

## Historical Notes

Prototype-specific historical guidance lives with those prototypes:

- `prototype-rust/AGENTS.md` documents egui focus/layout constraints.
- `prototype-swift/MOTIVATION.md` documents the AppKit focus motivation.
- `prototype-haskell/README.md` documents the parked Haskell/Wasm spike.
