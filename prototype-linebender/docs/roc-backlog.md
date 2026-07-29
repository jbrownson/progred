# Roc Back-Port Backlog

Working list. Delete it when drained; durable design decisions belong in
`docs/puri.md` and `AGENTS.md`.

Source: `~/git/puri-roc`, the third Puri implementation after Haskell and this
Rust version. The Roc project includes a continuation-based Clay port and a
complete Todo application, and received a close manual review of the Puri core.
Its `README.md`, `MOTIVATION.md`, and `ROC_NOTES.md` contain the resulting
design argument and language critique.

The implementations agree on the foundation:

- widgets retain no state and mint no identity;
- placement receives settled geometry and directly produces drawing and a
  transient handler;
- handlers are one-shot after a handled transition;
- newest registration tries first and decline falls through;
- hit testing is widget policy, not a retained region registry; and
- drawing is final-tagless, with a recording interpreter available when a
  frame is useful as data.

This file records only what the Roc work clarified or developed further.

## Translation rule

Roc cannot abstract over arbitrary mutable effects, so its callbacks explicitly
thread `state` and its rendering operations return monoidal placement results.
Rust already has the more direct representations:

- `&mut C` for application transitions;
- `&mut impl Canvas` for rendering; and
- mutable fields on a placement context for additional frame outputs.

Port policy and decomposition, not Roc's state/result plumbing, open tag
unions, formatter workarounds, or unnameable structural constraints.

---

# Port first

## 1. Back-port the clearer Puri argument

The Roc documentation now says the central idea more precisely than
`MOTIVATION.md` and `docs/puri.md` here:

- Puri sits below the choice of retained, immediate, React-style, or
  incremental state management. It is not itself immediate mode.
- Stable identity is history between evaluations, not a property of an output
  value. Snapshot reconciliation reconstructs provenance that its inputs did
  not contain.
- Explicit widget state removes Puri's need for cross-frame identity.
- Placement continuations remove the smaller within-frame need to correlate
  detached layout output with widget behavior.
- Puri is not intended to make the smallest UI take the fewest lines. It makes
  the real state/composition problem explicit so larger UIs acquire no
  accidental synchronization complexity.
- Keeping one application/UI state is the simplest consumer, not a Puri
  requirement. A retained tree, identity store, or reconciler can construct the
  same widget descriptions.
- An explicit UI state and change surface makes incremental-computation
  experiments possible without rebuilding text editing first.

Replace the claim that Puri simply "deletes the second stack" with this layered
account. Preserve the Rust/Linebender-specific implementation discussion after
it.

## 2. Add continuation-shaped `around` placement

Roclay's foundational combinator receives a settled placement and a
continuation that places the wrapped subtree. `before` and `after` are derived
from it.

Rust `layout::decorate` is only the before half. `handler::capture` already
provides the important handler operation, but no `Node<P>` combinator lets a
container naturally place its child inside that scope.

An `around` node should let its wrapper:

- perform work before and after the child;
- capture the child's handler and rebuild or discard it;
- gate or translate events before forwarding them; and
- state overlay/fallback precedence compositionally.

Direct canvas calls cannot be retracted after the continuation returns; this is
fine. The valuable interception target is the transient handler and the other
explicit fields of the placement context.

Derive `before` and `after` from `around`. Reconsider the name `decorate` once
both orders exist.

## 3. Make effective clipping part of placement

The Rust history treated a threaded clip rectangle as if it implied a retained
hit-region registry. The Roc implementation showed that these are independent.

A settled placement needs:

- the widget's complete layout rectangle; and
- the intersection still visible through ancestor clips.

This is ordinary geometry for the current pass. Nothing retains it, assigns it
identity, or centralizes hit testing. Widgets can still use arbitrary shapes or
ignore pointer position entirely.

This becomes necessary with nested viewports:

- ordinary presses and hover must not target clipped-away portions;
- drawing uses the same effective clip;
- drag motion and release deliberately remain unbounded after a gesture began
  inside; and
- entirely clipped widgets may skip rendering as an optimization.

Design this with `around` and the scroll widget. The current
`scroll::place_scrolled` clips ink but cannot generally communicate the
ancestor clip to descendant interaction.

## 4. Replace synthetic hover dispatch with explicit pass data

`App::refresh_hover` replays an artificial pointer move through the retained
handler after every mint. Hover callbacks mutate `App` while returning `false`,
contradicting `Handler`'s contract that a decline leaves the context unchanged.
The handler is then retained and reused because the mutation is declared
non-consuming.

There is no special category of hover state. The inputs are:

- the last pointer position, owned by the application; and
- settled widget geometry, recomputed by placement.

Simple widgets can compute hover directly from those inputs. Progred also needs
innermost precedence, occlusion, and gap hysteresis, so the placement pass can
emit a hover claim. Resolve that claim before the visible pass, retaining only
the genuine hysteresis state the policy needs.

A silent resolve pass followed by a visible draw pass is honest explicit
computation. It removes the synthetic event, restores the decline invariant,
and ensures the first presented frame already reflects layout changes under a
stationary pointer.

## 5. Split line-edit state from its description

`LineEditState` currently retains text interaction state together with
presentation inputs: font size, brush, prefix, and suffix. The Roc review
clarified that a widget description is ephemeral input for one placement, not
a retained widget object.

Separate:

- interaction state: selection, IME preedit, active selection drag, and
  whatever text custody the caller chooses; from
- description: current text, focus, font/style, affixes, placeholder,
  callbacks/capabilities, and other current presentation inputs.

The frame's handlers can close over the current description while obtaining
mutable interaction state and Parley contexts from `EditCtx`. This makes
restyling independent of editor lifetime and demonstrates the actual Puri
boundary more clearly.

Do not force focus and editor availability into one enum. A handler built while
focused may correctly find that the editor state no longer exists and decline;
the current `Option<EditCtx>` represents that stale-frame window intentionally.

## 6. Make clipboard access a supplied capability

`puri::edit` constructs `clipboard_rs::ClipboardContext` directly, while the
Progred shell implements richer structural and text clipboard behavior
separately.

Put text clipboard access on `EditCtx` or a small injected trait:

- Puri loses a platform dependency;
- editing tests can use an in-memory clipboard;
- the application chooses platform policy; and
- Progred can share one system pasteboard implementation.

Do not port Roc's explicit `state -> { state, text }` plumbing. That is a
language workaround, not part of the design.

---

# Small, high-value ports

## 7. Expand the `Interact` vocabulary

`puri::interact` currently exports only `clickable`, while `raw.rs` repeatedly
spells primary-button and rectangle checks.

Add the policy ladder used in Roc:

- primary pointer down with a placement-aware predicate;
- primary pointer down;
- primary click;
- ordinary clickable; and
- double-clickable.

Keep the variants thin and derive the simpler ones from the general one.

## 8. Add advisory keyboard-focus helpers

Port the policy, not focus ownership. The application supplies an explicit
ordered collection of entries and transitions; Puri may handle:

- Tab and Shift-Tab traversal with wrapping;
- unconsumed Escape clearing; and
- unconsumed primary-pointer clearing.

The helper draws nothing, stores nothing, and infers no order from the layout or
handler tree. More complex applications remain free to use nested focus zones
or another focus model entirely.

## 9. Make time an input

Attach a monotonic timestamp to translated events and provide a time-passed
input when time advances without another relevant event. This is the substrate
for caret blinking, delayed gesture policy, and animation without widgets
polling a clock.

Winit delivers events sequentially, so do not port RocRay's exact
batch-per-render API.

## 10. Extract and test the frame-remint protocol

The current shell has the right semantics:

1. dispatch into the frame the user saw;
2. if an event handles and mutates, discard that frame;
3. build the successor from the new state before another event can dispatch;
4. leave a genuinely declined frame standing.

The implementation is distributed across the window-event branch,
`retain_dispatch`, and `redraw`, and silent/visible frame construction is
duplicated. Extract enough of the protocol to test sequencing without trying to
hide winit, menus, or rendering behind a universal event-loop abstraction.

## 11. Add recorder coverage for widget geometry

`DrawList` already proves the final-tagless testing strategy. Extend it where
geometry matters:

- line-edit selection rectangles;
- caret geometry;
- clipping and horizontal scroll-to-caret;
- delimiter strokes and placement; and
- handler precedence around clipped subtrees.

The existing line-edit behavioral tests remain valuable; this covers the
rendering/placement half rather than asserting that strings merely pass through
draw calls.

## 12. Prefer named descriptions at large boundaries

Roc's named `Description` records made semantically different booleans and
callbacks visible at call sites. Rust should use ordinary structs where the
same issue appears.

The clearest candidates are:

- `raw::project`, whose thirteen arguments include adjacent values of the same
  type and which immediately constructs `Cx`; and
- `run_frame`, which suppresses `clippy::too_many_arguments`.

This is not a mandate to wrap every small function argument list.

---

# Port when Progred needs them

## Drag and reorder

The Roc decomposition is worth retaining:

- layout-independent invisible source, motion, and release handlers;
- caller-owned `Idle | Armed | Dragging` state;
- activation using settled row geometry;
- a pure reorder preview with a gap while the underlying collection remains
  unchanged;
- commit only on release; and
- a layout-specific reorderable-list adapter above the generic engine.

`Drag.hysteresis` is independently useful: withhold pointer moves until the
pointer leaves a radius around an origin. It handles the case where a held
pointer survives a large UI replacement without that replacement immediately
interpreting layout movement as a drag.

Do not add the list combinator until Progred has a reorderable structure.

## Click-run translation

When one widget is replaced partway through a multi-click run, a wrapper can
translate subsequent click counts into the new frame's reference. A physical
third click may be the new editor's logical double click; a raw single click
starts a new run and clears the adjustment.

Progred currently mounts editors on click one and has an ad-hoc fresh-count
rule, so this is not an active bug. Add the general combinator only with a
consumer, after `around` exists.

## Basic widget catalog

Roc now has small renderer/layout-independent button behavior plus checkbox and
text-button specializations. Port them when Progred needs conventional
controls. Do not import the Todo catalog merely to make Puri look complete.

## Bounded line editing

The Roc Todo demonstrates content clipping and horizontal scroll-to-caret in a
fixed-width single-line field. The current Progred inline editors size to their
content. Port the bounded behavior when a fixed-width field appears.

---

# Cleanup findings

- `caret_index` has already moved from `raw.rs` into `puri::text`; no work
  remains from the old audit item.
- `model.scroll` and `App::revealed` should not be collapsed merely because one
  resembles Roc Todo's old offset-plus-mode fields. `revealed` is edge-trigger
  memory for a selection-dependent effect, not an alternative representation
  of the offset. Redesign only with a clearer scroll-intent model.
- Keep `Handler` policy small, but its typed channels are appropriate for
  Rust's closed enums. Do not replace them with one global event enum.
- Audit mutable accumulators and nested matches only at concrete call sites;
  Roc's iterator and formatter limitations are not Rust lessons.

---

# Already present by another route

- Final-tagless `Canvas`, direct Vello streaming, `DrawList`, and replay.
- One-shot transient handlers with newest-first composition.
- Caller-owned text, focus, drag, scroll, and cache state.
- The measure/place split and continuation-bearing leaves.
- Caller-threaded text memoization rather than a framework cache.
- Domain identity (`Gid`/`Path`) distinguished from framework-minted widget
  identity.
- A single post-dispatch write-through path that commits active editing.

---

# Do not port

- Roclay as Progred's default layout engine. The baseline box algebra and
  Wadler grouping fit the document; Roclay remains evidence that the placement
  boundary can host another engine.
- Roc's monoidal placement-result plumbing. Rust's mutable generic placement
  context is the direct equivalent.
- Roc's pure state-threading handler signatures. `&mut C` is idiomatic and
  avoids cloning an application containing GPU resources and text caches.
- Roc's open structural event union. Typed handler channels are Rust's
  appropriate closed-world representation.
- Roc's generic scalar compromises. Keep kurbo's `f64`.
- Roc's custom UTF-8 line-edit engine. Parley supplies the stronger behavior;
  port the state/description boundary, not the implementation.
- RocRay platform workarounds, event batching, or silent renderer records.
  Their portable lessons are already listed above.

## Compiler caveat

Roc's optimized build becomes pathological on the higher-order Puri/Roclay
graph. This is a compiler problem, not a design verdict. Rust also
monomorphizes `Node<P>` and generic canvases, so watch compile-time growth, but
do not preemptively weaken the architecture.
