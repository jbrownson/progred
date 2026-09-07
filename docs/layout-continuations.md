# Layout and widget continuations

Current architecture, 2026-09-07.

## Boundary

Layout places boxes with opaque continuations. Widgets written in the Puri
style need not live in the Puri package: reusable controls belong in
`puri-widgets`, while document-aware widgets and adapters belong in Progred.
Projection, navigation, selection, hover policy, and event interpretation are
not box primitives.

The reference is tag `archive/pre-root-promotion`:

- `prototype-haskell/halay/src/Halay.hs`: `Measured` pairs size with
  `Placement -> placeM placed`; leaves and decorators contribute continuations.
- `prototype-haskell/puri/src/Puri/Widget.hs`: a widget takes settled placement
  and returns rendering plus handlers.
- `prototype-haskell/progred/src/Progred/Projection.hs`: `descend` resolves a
  location and calls the projection, rather than emitting a layout opcode.

The sibling `puri-roc` reference reinforces this: `package/Frame.roc` combines
placement output with a handler, `package/Handler.roc` composes one function over
input events, and `package/ScrollView.roc` wraps a child placement continuation.
Its Roclay bridge inserts widgets as leaves and wraps them with `around`.

We retain Progred's baseline box algebra and ordered alternatives, not Clay,
and preserve its separate settled-geometry hover stage.

## Frame construction

1. Partials produce `Layout<World, Hover>`, above the total structural fallback.
   Native functions supply controls and projection recursion.
2. Preparation runs those functions and composes the box measurements into a
   `ChoiceLayout<Fragment<World, Hover>>`. Resolving choices for the available
   width produces one `Measured<Fragment<World, Hover>>`.
3. Placement calls only the chosen continuations with their full rectangle and
   effective enclosing clip. They contribute hover probes, deferred painting,
   handlers, navigation landmarks, and floating subtrees.
4. The frame raises floaters, then resolves hover against the settled geometry.
5. Render continuations receive the resolved ink inputs and draw directly into
   the chosen canvas backend. The shell retains the handlers and navigation
   data for input dispatch.

Navigation stops are declared by projection/widget functions; placement only
supplies their rectangles. A control's arrival override is consumed by its
nearest navigation landmark. Discarded alternatives contribute no navigation,
hover, handlers, or painting.

## Ownership and types

[`display/src/measure.rs`](../display/src/measure.rs) interprets only box
composition and opaque leaf/preparation functions. `Row`, `Col`, `Overlay`,
`Pad`, `Surround`, `Floating`, sharing, and alternatives own geometry.
`Before` and `After` compose placement work without interpreting it.
`Surround` measures arbitrary side widgets against its chosen child's span;
`Floating` receives a positioning function and does not install popover policy.

A `Widget` prepares a `Measured<Fragment>`. A `Program` can instead prepare
a subtree with choices, using the same per-frame `ChoiceBuild`. Neither is a
catalogue of control variants. Puri's plain text/drawing leaves are measured by
[`widget::drawing`](../display/src/widget/drawing.rs); they contain no document
or interaction information.

[`Fragment`](../display/src/widget/frame.rs) is the one shared placement
output. The app's `Placed` is an alias, not a second representation with an
adapter. It combines rendering, a single function-over-`Event` handler chain,
hover probes, navigation, view regions, exact completion offers, and floaters.
This editor-facing output belongs to `progred-display`; Puri itself remains
layout-neutral and knows no document paths.

[`CanvasSink`](../ui/puri/src/draw.rs) is the object-safe primitive drawing
interface implemented by Vello, Canvas2D, and the test recorder. `Canvas`
supplies generic convenience methods above it. Whole-widget render closures
call that interface directly; there is no backend-specific fragment conversion
or new GID drawing list. Canvas clips use scoped boxed callbacks. Existing
initial encodings remain available where recording is intentional.

## Projection recursion and controls

`descend`, `at`, and `transient` build ordinary preparation functions using
an explicit [projection scope](../display/src/widget/project.rs). The app
supplies source lookup, cycle detection, provenance, fuel, and the selected
current/descendant partials. There are no corresponding Layout enum cases,
and the editor no longer pattern-matches on Layout.

Preparation still interleaves projection and measurement. These functions run
before choice resolution and placement, not eagerly while a partial first
constructs its description. That retains sharing: a descendant referenced by
several alternatives prepares once in that frame. The next frame starts a fresh
builder. No cross-frame cache or additional layout search was introduced.

`current` and `default` remain separate explicit projection inputs. Missing
locations pass `None` to the chosen partial and fall back to the ordinary
pending widget; computed roots remain read-only. The GID layout library's
path-bearing forms decode through the common path library into these functions,
not a second path opcode interpreter.

Line editing, delimiters, pointer actions, hover feedback, scrubbing, state
scrolling, borders, and popovers are native widget functions/combinators.
Completion and drawing-program functions request scoped app adapters during
preparation; the adapters own document-specific offers/evaluation/source
attribution and return the same measured widget output. They are not operations
of the box interpreter. Completion providers remain explicit lazy inputs and
run only for the active picker.

Native widgets request site state and editing capabilities only when needed.
Inert decorations do not construct text state or Grap interpreters. Conversion
callbacks belong to the current line handler, never persistent selection.
Event acceptance is independent of whether state changed; scroll handlers return
unused displacement explicitly. Scroll, clipping, floating, and navigation
composition are shared rather than duplicated between native and editor output.

## Verification

Regression tests cover contextual projection scopes, missing locations,
computed roots, source-qualified follows, completion, editing/conversion,
gesture/view ownership, clipping, navigation, floaters, and shared preparation.
Projection-shape tests use a test-only recording implementation of the recursion
interface; no parallel request enum is retained in production.

The frame canaries in [performance.md](performance.md) exercise complete
headless frames, including disposal. They are regression checks, not interactive
frame-rate measurements. Completion ranking remains independently
[deferred](deferred.md#completion-ranking).

The final check passed 536 affected unit tests and all seven release canaries,
the native workspace/all-target check, and the wasm32 library check. The latter
still reports the pre-existing unused web menu field/variant warnings. The
headless sample and Grap SVG renderings were also inspected without launching
the app.

Final-boundary comparison against the preceding committed checkpoint, with
five warm-up and 60 measured frames (median frame plus disposal):

| Workload | Before | Unified output and preparation |
| --- | ---: | ---: |
| IoP source | 4.18 ms | 3.90 ms |
| IoP picture | 24.38 ms | 23.74 ms |
| Fidget orbit | 9.26 ms | 10.11 ms |
| Torus | 6.77 ms | 7.82 ms |
| Tanglecube | 44.78 ms | 46.33 ms |
| Gyroid | 29.38 ms | 30.55 ms |
| Fidget cube | 13.91 ms | 15.16 ms |

Source p95 was 4.24 ms, versus 4.43 ms before. The GPU-oriented canaries vary
more between runs; these are not controlled speedup or slowdown attributions.
An earlier run of the shared-output boundary measured Fidget at 9.00 ms and
Torus at 6.90 ms. No search policy or Fidget rendering algorithm changed.
Unified event handlers still visit unrelated registrations; the opt-in
`handler_dispatch_profile` retains a focused check of that interface cost.
