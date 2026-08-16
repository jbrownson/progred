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
- placement receives settled geometry and directly produces drawing, a
  transient handler, and other explicit frame outputs;
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

# Port when Progred needs them

## Advisory keyboard-focus helpers

Port the policy, not focus ownership. The application supplies an explicit
ordered collection of entries and transitions; Puri may handle:

- Tab and Shift-Tab traversal with wrapping;
- unconsumed Escape clearing; and
- unconsumed primary-pointer clearing.

The helper draws nothing, stores nothing, and infers no order from the layout or
handler tree. More complex applications remain free to use nested focus zones
or another focus model entirely.

The inference ban has a history worth keeping. Roc's `Handler` originally
carried focus traversal inside the monoid — first, last, next, previous, and
whether the subtree held focus, combining as handlers composed — which yields a
Tab order derived from the placement tree for free. It was deleted. A derived
order is the identity argument from `MOTIVATION.md` in another costume:
presentation refactoring that changes nothing a user can see would silently
change traversal behavior.

## Time and wakeup as inputs and outputs

Attach a monotonic timestamp to translated events when a consumer needs time.
A widget that needs progress without another event must also emit its next wake
deadline as placement output; the shell owns the timer and supplies the next
time input. This is the substrate for caret blinking, delayed gesture policy,
and animation without widgets polling a clock or forcing a continuous loop.

Winit delivers events sequentially, so do not port RocRay's exact
batch-per-render API.

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
rule, so this is not an active bug. Progred's general `around` wrapper is present;
use it for the translation only when the interaction needs it.

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

# Landed or already present

- Line-edit interaction state separated from its per-frame presentation:
  font, brush, affixes, focus, placeholder, and chrome can change without
  remounting cursor, selection, drag, or IME state.
- Text clipboard access supplied through `EditCtx`; Puri has no platform
  clipboard dependency and its editing tests use an in-memory implementation.
- The placement-aware primary-pointer policy ladder in `puri::interact`, down
  through ordinary and double click specializations; Raw uses it for toggle,
  insertion, and command-pick targets.
- Named descriptions at the large frame and Raw projection boundaries, making
  semantically different selections, hover projections, modes, and resources
  explicit at call sites.
- Recorder/geometry coverage for line-edit selection and caret rectangles,
  nested clips, delimiters, and clipped handler precedence.
- Continuation-shaped `around` placement in Progred's box algebra, with
  `before` and the old rectangle-only `decorate` derived from it.
- Explicit full `rect` and enclosing `clip_rect` values on every placement;
  nested scroll viewports intersect, gate gesture starts and hover, and leave an active
  gesture's motion and release unbounded.
- Real scroll registrations on the transient handler tree for both the
  document viewport and graph camera; no shell geometry fallback.
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
monomorphizes `Measured<P>` and generic canvases, so watch compile-time growth, but
do not preemptively weaken the architecture.
