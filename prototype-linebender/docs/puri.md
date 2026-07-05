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

A pure widget library: rendering and behavior, nothing else.

- A widget is a pure function from (persistent widget state, props) to
  (draw calls, handlers).
- State a widget must keep across frames — cursor/selection, scroll
  offset, drag state, focus — is defined by Puri as types and passed in
  by the caller. Puri holds nothing between frames.
- Every input event runs the pure pass fresh (no scene attached),
  collecting a transient `Handler` that is dispatched once for that
  event and discarded. Nothing is retained across events, so dispatch
  geometry always matches the displayed frame — purity guarantees the
  re-run reproduces it. The pass itself stays read-only in the model;
  all mutation happens in dispatch, after placement completes,
  preserving one-event-one-transition and avoiding read-after-write
  order dependence within a pass.
- A `Handler` holds one composed function per event kind (typed
  channels: pointer down, key — extended as widgets need). The monoid
  is function composition, mirroring how rendering works: `on_*` wraps
  the existing function so the newest dispatch tries first and declines
  fall through; mempty declines everything. No Vec, no dispatch policy
  — ordering is the composition. There is no
  region registry; widgets gate by their own settled rects inline, so
  non-rectangular picking is first-class. `capture` scopes a subtree's
  registrations into a value its parent composes — call, wrap with
  before/after, transform events, or drop. If per-event passes ever
  cost too much (text shaping per mouse move), the answer is the
  planned caller-side memoization, not retained dispatch. No action
  type or reducer is baked in; per-widget action vocabularies (the line
  edit's) exist for testability without any global action enum.
- Puri mints no identity and retains no hierarchy. The widget tree is a
  function of the app model every frame; parent/child relationships are
  never a parallel state that needs syncing.

Deliberately out of scope: state management, reactivity, identity,
layout engines, styling opinions, widget catalogs.

The thesis: UI is hard because there are two stacks of state — the
app's and the toolkit's — and React reconciliation, egui `Memory`, and
focus-sync bugs are all costs of keeping them aligned. React makes the
syncing cheaper; Puri deletes the second stack. Widget behavior then
survives any state-management regime, so state management can be
experimented with separately without rebuilding the text box each time.

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

## Layout

No general layout engine. Three layers, smallest sufficient model:

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

The placement interface keeps measurement and placement separate
(Halay's shape: measure/place split, opaque leaves, placement callbacks
receiving settled rectangles that produce draw calls and interaction
records). Clay or Taffy could implement the same interface later as
adapters if some subtree earns declarative flex; neither is a
dependency now.

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
  the reference for one. Editor state is a caller-owned value either
  way: the contract is custody, not representation.
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
boxes are `pad`/`decorate` composition), which costs nothing: what
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
   transitions, box algebra, placement interface), puri-vello, progred.
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
the smaller, sufficient model. The placement interface is unchanged, so
the Clay/Taffy adapter option remains open without being a dependency.

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
transient Handler of composed dispatch functions, rebuilt by a fresh
pure pass for every event, dispatched once, discarded. Freshness like
egui's run-per-event model; phase separation unlike egui's fused one.
Refined same day at the user's direction: the per-kind Vec channels
became one composed function per kind (a unified Event enum was tried
for a moment and rolled back — typed channels, composed accumulation),
and `Canvas` clipping became a scoped closure (`clip(shape, t, |c|
...)`) so unbalanced push/pop is unrepresentable — the recorder gains
nested `Clip { children }` structure in the bargain.
