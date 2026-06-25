# Rust Pivot Handoff

Date: 2026-06-25

## Decision

Pause the Haskell/Wasm spike and move the next Progred prototype toward
native Rust.

The point is not to discard the Puri idea. The point is to stop paying
Haskell/Wasm costs when the prototype is no longer clearly using the
Haskell-specific advantages that justified those costs.

## Where The Haskell Spike Stopped

The hover/debug spike was abandoned and reverted from the workspace
because it was not working reliably in Tauri. Tests and `make dist`
passed after the last attempted changes, but the behavior was not
manually confirmed before pausing, so the code was not worth keeping.

The reverted work had attempted to add:

- `Editor` hover state threaded into projection/rendering.
- Compose hover drawing for raw labels/nodes and projected list rows.
- A JS/Tauri modifier cache so `Cmd`/`Ctrl` state survives pointer events
  that do not report modifier fields correctly.
- Debug logging from both JS and Haskell for pointer moves and hover state.
- Some cleanup around no-op renders and list pending compose states.

The work also exposed a design smell: the recent interaction layer grew
awkward hit-testing/debug machinery while still not making the behavior
obvious. Treat that implementation as spike evidence, not as something to
port directly.

## Why Switch

Haskell was attractive because higher-kinded types and final-tagless
style could support a Purview-like UI architecture. In practice, this
prototype has mostly been spending effort on:

- Wasm build and packaging details.
- JSFFI and browser/Tauri event forwarding.
- Sparse Haskell graphics bindings.
- Debugging pointer/focus behavior across a canvas boundary.
- UI interaction plumbing that is not yet paying for its abstraction cost.

The Wasm target was a workaround to access browser canvas as a vector
graphics renderer. It was not the desired product shape. The next
prototype should be native.

Rust changes the tradeoff:

- Better native windowing and rendering ecosystem.
- Direct access to `winit`, `wgpu`, Vello, Skia bindings, tiny-skia, and
  platform integration crates.
- Larger GUI community and more adjacent projects to compare with or
  contribute to.
- Less runtime and toolchain friction for native app iteration.
- Enough type-system support to express the useful parts of Puri without
  requiring the full Haskell abstraction stack.

## What To Carry Forward

Carry forward the Puri shape, not the exact Haskell implementation.

The valuable idea is:

1. The app model is plain data.
2. Projection code derives a UI from graph/model state.
3. Layout computes concrete rectangles.
4. Rendering consumes a backend-independent draw list.
5. Interaction is declared near the UI element that owns it.
6. Events reduce to explicit app actions.

Do not carry forward the recent ad hoc hit-region layer as-is. The Rust
prototype still needs picking, but it should be designed deliberately:
either placement callbacks produce interaction records alongside draw
commands, or stable element IDs let the layout pass expose bounds that a
central picker can query. The important constraint is that picking should
be inspectable and deterministic.

## Native Rust Direction

Start with a native shell, likely `winit`, and bias toward trying Vello
first. Vello is promising and aligned with Rust-native vector rendering,
but it should sit behind a renderer boundary because it is still a moving
target and the app should not be coupled to any one graphics stack.

A reasonable split:

- `progred_core`: graph model, focus/editing model, projection decisions.
- `progred_ui`: Puri-like layout/view primitives, backend-independent
  draw commands, input actions, picking/debug data.
- `progred_layout_clay`: optional Clay or `clay-layout` adapter.
- `progred_renderer_vello`: Vello renderer for the draw command set.
- Alternative renderers later: `tiny-skia`, `rust-skia`, or a platform
  renderer.

Keep the renderer boundary boring. Prefer an enum draw list such as
rectangles, borders, paths, glyph runs/text, clips, and transforms before
inventing a highly generic rendering abstraction.

## Clay

Clay is still relevant, but it should be treated as a layout candidate,
not as the whole UI system. The Rust bindings exist as `clay-layout`, and
the upstream Clay model is close to the row/column/flex-like layout we
have been exploring.

The potential mismatch is API shape. Clay is oriented around declaring
elements between begin/end layout calls and producing render commands.
Puri wants a placement-style API where layout placement can also produce
app-specific output. The first Rust spike should try an adapter before
forking or modifying Clay.

## Final Tagless In Rust

Final tagless partly makes sense in Rust, but not in the full Haskell
sense.

Rust can express a useful first-order version with traits:

```rust
trait Ui {
    type Node;

    fn text(&mut self, text: &str) -> Self::Node;
    fn row(&mut self, children: impl IntoIterator<Item = Self::Node>) -> Self::Node;
}

fn inspector<U: Ui>(ui: &mut U) -> U::Node {
    let title = ui.text("Inspector");
    ui.row([title])
}
```

That lets components be generic over interpretation: render tree,
snapshot tests, accessibility extraction, debug dumps, etc.

The trouble starts when trying to reproduce Haskell's higher-kinded,
effect-polymorphic style. Rust has traits, associated types, and generic
associated types, but not Haskell-style HKTs/typeclasses. Lifetimes,
object safety, closure ownership, and monomorphization can quickly make a
literal final-tagless port feel heavier than the problem.

For this project, the pragmatic Rust version is probably:

- Use traits/builders where they remove duplication.
- Use explicit enums for draw commands, actions, focus state, and model
  edits.
- Keep generic interpretation local and measurable.
- Avoid type-level architecture unless a concrete second interpreter
  exists and is pulling its weight.

## First Rust Acceptance Test

The next prototype should prove this before growing:

1. Native window opens.
2. Vello renders a small graph/tree projection.
3. A layout pass produces rectangles for visible UI elements.
4. Hover/focus/compose behavior works for raw graph rows and a list
   projection.
5. A debug overlay can show layout bounds, picked element, focus state,
   and generated action without using external console inspection.

If that is simpler than the Haskell/Wasm path, the pivot is justified.
