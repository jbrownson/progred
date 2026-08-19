# Puri: UI Runtime Decision

Date: 2026-07-03. Originally written in `prototype-rust/docs/` planning
a Clay layout shim; revised the same week when the box-algebra layout
decision replaced it and the focus, caching, and Masonry-salvage
contracts were settled. The earlier reasoning is preserved in History
at the bottom.

## Decision

Build the native Rust prototype on Puri, a pure widget library carrying
forward the Haskell spike's design, instead of continuing with egui.

egui's original showstoppers are gone: egui 0.35 fixed the `lost_focus`
transfer bug, and raw `Sense::CLICK` keeps click targets out of the Tab
ring. The remaining objection is the model itself. egui is not actually
stateless — focus, cursors, scroll offsets, and collapse state persist
in `Memory`, keyed by widget IDs the app does not control — and its fix
pattern for ordering bugs is to defer events to later frames, which
turns correctness bugs into visible intermediate frames (the
focus-highlight flash before a popup arrives). Keeping `TextEdit` while
rejecting egui's state model would preserve the exact seam that
produced the old focus bug class.

## What Puri Is

A pure widget library: rendering and behavior, below the choice of state
management.

- A widget is an ephemeral description constructed from current inputs.
  Placement consumes that description with settled geometry and RETURNS a
  value (2026-08-18, the Halay shape restored): the layout engine folds
  leaf contributions with a caller-defined monoid whose combine names its
  asymmetry (`base.over(above)` — placed later, painted on top, asked
  first), so the engine knows nothing of what placement produces.
  Progred's instance carries hover probes, the composed handler,
  keyboard geometry, the popup, and DEFERRED ink; rendering runs the ink
  when the caller chooses, so a silent dispatch mint never draws.
- State a widget must keep across frames — cursor/selection, scroll
  offset, drag state, focus — is defined by Puri as types and passed in
  by the caller. Puri holds nothing between frames.
- Each pass yields drawing and placement outputs including a `Handler`. The
  handler is a pure function of the state the pass read, so it is
  SINGLE-SHOT with respect to mutation: the shell retains it, events
  dispatch into it, and the first handled (mutating) event spends it —
  the shell mints the successor from the mutated state immediately, in
  the event path, so no later event (even in the same gesture) ever
  dispatches into a spent handler. A changed frame input also mints a
  successor; only a genuinely declined event whose inputs remain unchanged
  leaves the frame standing. The redraw derives the pixels from the same
  state. This was settled 2026-07-07 after visiting both wrong corners
  the same day: fresh
  pass per event dispatched against state NEWER than the pixels
  (a stepping simulation made quick clicks miss what they aimed at)
  even when the event changed no frame input; retain-until-vsync let
  same-gesture events dispatch into a handler whose state was gone (a
  deselect followed by a drag-move panicked the editor hook). Residual windows
  where mutation happens outside dispatch (menu commands, pinch)
  remint only at the next redraw, so handlers must still decline on
  absent state rather than assume it — the editor hook returns Option
  for exactly this. The pass itself stays read-only in the model; all
  mutation happens in dispatch, preserving one-event-one-transition
  and avoiding read-after-write order dependence within a pass.
  Handlers remain shell custody, never puri's.
- Pointer position is ordinary frame input, and hover is DERIVED per
  pass, never stored: placement's probes are asked what the pointer
  rests on (`puri::hover::Claim` — a claim names a target or occludes,
  the claim analog of an opaque fill; no answer is air), air defers to a
  `LazyPointer` ring whose trailing center is the little-gap hold as a
  dead-zone filter on the INPUT rather than remembered footprints, and a
  pressed gesture keeps the hover it began with. Hover-conditioned paint
  reads the resolved answer from the render pass's ink context, so the
  frame is built hover-blind and the first presented frame agrees with
  its own hover by construction — no silent resolve pre-pass, no
  invalidation bookkeeping, no fixed-point redraw. History: hover
  callbacks that mutated on decline, then synthetic motion replay, then
  a stored hover resolved by a doubled pass per redraw — each fell to
  the same lesson, that hover is a pure question of settled geometry
  asked between placement and ink.
- A `Handler` holds one composed function per event kind (typed
  channels: pointer down, key — extended as widgets need). The monoid
  is function composition, mirroring how rendering works: `on_*` wraps
  the existing function so the newest dispatch tries first and declines
  fall through; mempty declines everything. No Vec, no dispatch policy
  — ordering is the composition. There is no
  region registry; widgets gate by their own settled rects inline, so
  non-rectangular picking is first-class. `capture` scopes a subtree's
  registrations into a value its parent composes — call, wrap with
  before/after, transform events, or drop. No action
  type or reducer is baked in; per-widget action vocabularies (the line
  edit's) exist for testability without any global action enum. Thin
  `interact` helpers factor the common placement-visible primary-down,
  click-count, ordinary-click, and double-click policies while leaving
  state transitions and decline with their callers.
- Puri mints no identity and retains no hierarchy. A widget description
  contains no provenance from prior evaluations; if its consumer needs
  identity, that history belongs to the consumer.

Deliberately out of scope: state management, reactivity, identity,
layout engines, styling opinions, widget catalogs.

The thesis is layered rather than a prescription for one application state.
Stable identity is history between evaluations, not a property of an output
value. Reconciliation and retained identity stores are ways to supply that
history; Puri neither requires nor forbids them. Its narrower contract is that
widget rendering and behavior do not secretly custody another copy of the
state. Progred chooses one explicit application/UI model because it is the
simplest consumer. A retained tree, React-style reconciler, or incremental
computation system can construct the same descriptions without rebuilding
the text box.

Consumer-owned placement continuations remove the smaller within-frame
association problem: layout can settle a rectangle and immediately continue the description whose
behavior belongs to it, without minting an ID and correlating detached output
later. Puri is not optimized for making the smallest UI take the fewest lines;
it makes the real state and composition surface explicit so synchronization
complexity does not appear accidentally as the application grows.

## Contracts

Focus:

- `focused` flows in as ordinary widget state/props. The app owns who
  has focus, tab order, and when focus moves; Puri never requests or
  transfers focus.
- Puri may ship pure helpers (ordered focusable keys plus current →
  next/previous) but they are advisory.
- The focused text widget emits the caret rectangle as output so the
  app can forward it to winit for IME candidate-window positioning.

Caching:

- No framework caches. Immediate-mode toolkits prove per-frame
  recomputation is viable; start there.
- The anticipated exception is text shaping: a caller-threaded memo
  table keyed by (text, style, width) — transparent memoization of a
  pure function, owned and passed by the caller like any other state.
  The same pattern, one level up (memoized projection subtrees), is the
  future incremental-computation hook. Neither exists until profiling
  demands it.

## Layout Boundary

Puri owns `Placement`, the settled geometry supplied to a widget, but no
layout node, traversal, or container. Text and line-edit descriptions expose
their measured metrics and accept an explicit placement; interaction helpers
register against one. A consumer can use Clay, Taffy, a retained layout tree,
or no general engine without changing those widgets.

Progred currently uses three layers, smallest sufficient model:

- Document content uses a small box algebra with baselines (the
  TeX/pict model): a box is (width, ascent, descent, draw); a line is
  horizontal composition on baselines; line height is max-ascent plus
  max-descent. Multi-line constructs inside a line (an equation in a
  list row) are vertical boxes with a chosen baseline — the founding
  case of the model, not a corner case. This also retires the egui-era
  block-in-inline problem by construction.
- A Wadler-style grouping pass above decides flat-versus-broken, needing
  only a fits-in-width oracle from the box layer.
- App chrome (panels, toolbars) is a few hand-coded flex-ish containers.

Progred's placement interface keeps measurement and placement separate
(Halay's shape: measure/place split, opaque leaves, placement callbacks
receiving settled rectangles that produce draw calls and interaction
records). Clay or Taffy could implement the same interface later as
adapters if some subtree earns declarative flex; neither is a
dependency now.

Scrolling (2026-07-05, revised 2026-08-03): the shell owns the offset
as ordinary app state. Progred's `layout::place_scrolled` shifts the child inside a
canvas clip, derives the shifted child's `clip_rect` from the explicit
viewport placement, captures the child's transient handler, and installs it
behind the viewport's own scroll action. Pointer-down and nested
scroll starts are bounded by the viewport; pointer motion, release,
keyboard, and IME remain available so a gesture or editor that began
inside can finish outside. The graph camera registers on the same
scroll channel after the document, so ordinary newest-first handler
composition expresses their visual precedence without shell-level
rectangle dispatch.

Every Progred layout leaf and wrapper receives Puri's settled `Placement`, carrying the widget's
full `rect` and effective enclosing `clip_rect`: the intersection of ancestor
axis-aligned layout clips, not pre-intersected with the widget. Ordinary child
placements inherit it unchanged; only an actual clipping container intersects
its bounds into the clip. Visibility is `rect.intersect(clip_rect)`, computed
when needed, and hover and ordinary press policy require the point inside both.
The clip is explicit placement data, not ambient mutable context.
Canvas clipping remains the independent ink mechanism and may use
arbitrary shapes; `Placement::clip_rect` neither describes nor replaces
those shapes. A fully clipped subtree is still placed because it can
still participate in hover resolution, navigation, and transient handler
construction; culling is valid only for work known to be dispensable.
Progred's `around` supplies the settled placement and an owned, one-shot
`PlaceInner`; calling `place_inner.place(ctx)` realizes the wrapped subtree.
`before` and rectangle-only `decorate` are its common orderings. These are
consumer layout tools, not part of Puri's widget API.

The first revision rejected this, reading the Haskell spike's
threaded Placement as "the same retained-region wrongness the handler
redesign removed." That conflated two different things. A retained
region registry stores geometry ACROSS frames and attaches identity
and dispatch policy to it; an effective clip is ephemeral geometry
for one placement, retained by nobody, naming nothing, and leaving
every widget free to use another hit shape or ignore the pointer
entirely. Rejecting the registry was right; rejecting the geometry
was not. The Roc implementation carries the clip rectangle in its
placement throughout and shows the two are independent.

## Stack

- winit for windowing, input, and IME events; ui-events as the portable
  event vocabulary (pointer positions arrive in physical pixels,
  matching placement coordinates), with ui-events-winit as the shell's
  adapter.
- Vello behind the renderer boundary, as the only backend. Drawing goes
  through a final-tagless `Canvas` trait (fill, stroke, glyph run,
  clip); drawing code is generic over the canvas and takes state as
  parameters, so Puri knows neither the backend nor the app model. The
  trait's vocabulary stays concrete — `Shape` over kurbo, peniko
  brushes, `GlyphRun` — so recordings keep their identity (a rect
  records as a rect, not a bezier soup).
- `DrawList` is the recording interpreter of `Canvas`, and `replay`
  plays a recording back into any canvas. peniko provides styling
  vocabulary and kurbo geometry, but neither is a display list, and
  `vello::Scene` is a write-only GPU encoding; the recorder is where
  frames become inspectable data when data is wanted — tests, goldens,
  and future fragment caching — while the vello canvas streams with no
  intermediate allocation.
- Testing interprets: surgical test canvases where a property is
  enough, the recorder where asserting on data is clearer (numeric,
  diffable — the same spirit as Halay's numeric conformance oracle).
  Occasional visual goldens use vello's headless render-to-texture
  readback; single-machine determinism is sufficient for this repo. If
  a CPU rasterizer with vello-identical semantics ever matters,
  vello_cpu is the family answer once it matures. An earlier revision
  planned a tiny-skia second backend for golden images; cut because
  tiny-skia has no text stack and recorded frames cover the regression
  need better.
- Parley for text layout; its `PlainEditor` as the line-edit engine or
  the reference for one. `LineEditState` retains only text and
  interaction state; `LineEditDescription` supplies the current font,
  paint, affixes, focus, placeholder, and chrome. Its handlers capture
  that presentation while `EditCtx` supplies mutable state, Parley
  contexts, and the application's `TextClipboard` capability at
  dispatch. Puri therefore depends on no platform clipboard library.
- AccessKit deferred. Identity is the caller's job, so accessibility
  IDs are too; Puri can emit accessibility content as placement output
  later.

## Behavior Sources

Masonry is the layer Puri parallels, not a foundation to fork. Its
contexts are short-lived per-pass views, but the retained caching lives
in `WidgetState`/arena (layout results, hover/focus flags) and root
state (focus, IME, pointer capture), and the invalidation protocol
(`request_layout` and friends) plus child-registration plumbing is
threaded through every widget body — Masonry is retained and
damage-driven, not per-frame.

How sourcing actually resolved (2026-07-04): the working quarry for
text turned out to be parley's own vello_editor example — Masonry's
textbox is PlainEditor plus tree plumbing, and vello_editor is the same
behavior without the plumbing. Keyboard semantics, IME handling,
selection/cursor geometry, and clipboard (clipboard-rs) all transplanted
from there. An earlier directive to mimic Masonry's controls was
superseded by the compositional-widget directive (bare text edit;
Progred supplies `pad`/`decorate` composition), which costs nothing: what
Masonry's boxed control bundles beyond vello_editor is exactly the
chrome that lives in the wrapper layer here.

Masonry remains a read-only reference for AccessKit whole-tree
integration patterns and behavioral policy (scroll-to-cursor feel in
fixed-width boxes, blink timing). Pointer capture is not borrowed at
all: with transient handlers, "captured" is a bool in caller state that
move dispatches consult. Nothing inherits Masonry's tree, pods, or ctx
protocol.

Audit against Masonry's actual source (2026-07-04): their architecture
converged with ours — `TextArea<const USER_EDITABLE>` is bare
PlainEditor text ("if clipping is desired, that should be added by the
parent widget") and `TextInput` is a wrapper widget, so the
mimic-Masonry and compositional directives agree. Same engine, same
event semantics, same decomposition. Their state we deliberately don't
carry: `rendered_generation` (damage tracking), `last_max_advance`
(relayout invalidation) — recompute-everything covers both, and
PlainEditor's `Generation` is available if memoization ever wants it.
Adoption backlog, to take from them when each arrives: blink policy
(cycle/timeout constants, cursor stops blinking after inactivity,
blink resets on every text event; elapsed-ms state fits time-as-input),
the `InsertNewline` policy enum for Enter semantics, hint-off during
animation, I-beam hover cursor, placeholder text in the wrapper, and
their IME-area refresh points (ours are covered by redraw-after-every-
handled-event; revisit if redraws are ever skipped).

## Linebender Strategy

Lead with the artifact. Build Puri standalone on
winit/Vello/Parley/kurbo/peniko, dogfood it in Progred, and keep quiet
until it works.

Once Progred runs on Puri: an essay (two-stacks thesis, atomic
transitions, placement-callback layout) plus a toy retained-mode shell
as an example consumer, posted to the Linebender Zulip. The shell is
also the acid test that state-management-agnosticism is real. The
endgame question — Xilem managing state directly over Puri widgets — is
theirs to pick up, on the strength of the demonstration, not a proposal
to open with.

The name stays Puri: it fits the Linebender family (kurbo and peniko
are Esperanto; pura is Esperanto for pure), and pgui is one letter from
gpui.

## Sequence

1. Workspace: puri (draw list, events, widget state types and
   transitions, placement geometry), puri-vello, progred (its box algebra).
   Graph/core crates copied from `prototype-egui/` as needed.
2. winit + Vello window drawing rects and Parley text.
3. Draw-list enum in puri with snapshot/property tests on the commands;
   puri-vello interprets it into a `vello::Scene`.
4. Box algebra with baselines behind the placement interface.
5. Handler dispatch ported from the Haskell Puri semantics.
6. Single-line edit with winit IME preedit (transplanted from parley's
   own vello_editor example, which proved framework-free and closer
   than Masonry's textbox). Bare text as the widget; boxes are
   composition.
7. D-tree projection from the graph/core crates on top.

Gate: the acceptance test in `prototype-haskell/RUST_PIVOT.md`,
unchanged. Tripwire: if single-line editing is not real after roughly a
month of normal pace from the date above, reconsider against the egui
shell in `prototype-egui/`, which stays in-tree as the fallback.

Guard: extend Puri only as Progred needs it. The ecosystem contribution
is a byproduct of it existing, not a requirements source.

## History

The first revision of this document planned layout as the Rust Clay
bindings shimmed behind the placement interface via per-frame temporary
IDs, with a Taffy shim later as proof the interface was not
Clay-shaped. Superseded 2026-07-03: box constraints is a protocol that
can host such engines as container implementations, but Progred's
document body is pretty-printer-shaped, and the baseline box algebra is
the smaller, sufficient model. Corrected 2026-08-03: that algebra initially
lived in Puri and made `Node` the return type of text, editing, and interaction
helpers. It now belongs to Progred; Puri exposes measurement and explicit
placement, so the Clay/Taffy adapter option remains open without changing a
widget API.

Drawing was first specced as a mandatory draw-list value with backend
interpreters. Revised 2026-07-03 to the final-tagless `Canvas` trait
(the tame, first-order version RUST_PIVOT endorsed): passing state as
parameters rather than closing over it removes the closure/borrow
objection, generics remove the object-safety objection, and the
recorder plus `replay` keep every frames-as-data use without making the
allocation mandatory on the render path.

The handler layer went through four shapes in two days, all recorded
here. First an Elm-style reified action type (Model/Action/update) —
reverted at the user's direction: no global action enum (a standing
principle from the TypeScript prototype survey). Second, a rect-region
registry retained between frames — rejected: the retained hit-region
layer was precisely the part of the late Haskell spike that felt wrong
and was reverted there too. Third, an overcorrection that deleted the
Handler entirely and fused event consumption into placement — wrong by
the architecture's own oldest rule (the render pass is read-only; all
mutation happens after), since mid-pass mutation reintroduces
order-dependent behavior within a frame. The settled design: a
transient Handler of composed dispatch functions, one per rendered
frame, dispatched against until a handled transition or changed frame
input replaces it (2026-07-07; the interim rebuilt for every event and
hit-tested against state the user had not seen). Phase separation
unlike egui's fused model, and dispatch against the presented frame
unlike egui's run-per-event freshness.
Refined same day at the user's direction: the per-kind Vec channels
became one composed function per kind (a unified Event enum was tried
for a moment and rolled back — typed channels, composed accumulation),
and `Canvas` clipping became a scoped closure (`clip(shape, t, |c|
...)`) so unbalanced push/pop is unrepresentable — the recorder gains
nested `Clip { children }` structure in the bargain.
