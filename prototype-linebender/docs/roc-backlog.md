# Roc Port Backlog

Temporary. Delete when drained — this is a working list, not a
decision record.

Source: `~/git/puri-roc`, a third Puri implementation (after Haskell
and this one) built 2026-07-21..28 on Roc, with Roclay (a
continuation-based Clay 0.14 port) for layout and a complete Todo
example. Its `README.md`, `MOTIVATION.md`, and `ROC_NOTES.md` carry
the design argument and the language friction.

The three implementations converged on the same core — one-shot
handler per presented frame, newest registration tried first, decline
falls through, no region registry, no minted identity, final-tagless
canvas, placement continuations receiving settled rects. Roc's
`EventLoop` (rebuild per event, silent canvas for intermediate
frames) and this repo's single-shot-with-remint are the same
semantics reached from opposite directions; `retain_dispatch`'s
`scene: None` is literally `Canvas.silent`. What follows is only
where Roc went further.

Part I is capability to port. Part II is the simplification pass the
Roc source received and this one has not — read it as an audit
checklist, not a work order.

## The filter to apply to everything below

Roc has no `&mut` and no ST monad, so EVERY Roc interface threads
`state` explicitly: `Handler`, `Button.Action(state) : state =>
state`, `Drag.Begin(state) : state, Placement, PointerButtonEvent =>
state`, `Clipboard.read! : state => { state, text }`. That is Roc
compensating for a missing language feature, not a design finding.
Rust's `&mut C` is the better answer and none of that plumbing should
cross over.

So for each item, separate three things:

1. the POLICY (what should happen when the user does X),
2. the DECOMPOSITION (which module owns which decision), and
3. the STATE PLUMBING (how the transition is expressed).

The first two travel. The third does not — and neither does anything
Roc did to work around `var`, missing iterator combinators, or its
formatter. Items below are worded to carry policy and decomposition
only; where an item is at risk of smuggling plumbing across, it says
so.

---

# Part I — Capability

## 1. `around` as a layout combinator

Roc: `roclay/Roclay.roc:141`, `RoclayInternal.roc:262-281`,
`puri-roclay/Layout.roc`.
Here: `puri/src/layout.rs`.

`Kind::Decorate` runs `draw(ctx, rect)` and then places the child —
that is Roclay's `before` and nothing else. Roclay derives both
halves from `around`:

```roc
before = |place!, layout| around(|placement, place_inner!| place!(placement) + place_inner!(), layout)
after  = |place!, layout| around(|placement, place_inner!| place_inner!() + place!(placement), layout)
```

`around` receives the settled placement plus a continuation that
places the node's content and returns its `Frame` — so the wrapper
holds the subtree's HANDLER as a value and can call it, wrap it,
rewrite events for it, gate it, or drop it. That is the operator
`Interact.adjust_click_run` and `ScrollView.vertical!` are both
built on.

For ink, `around` is derivable here from before + after: ordering is
call order, no result to sequence. For HANDLERS it is not. Ours
compose into a shared `&mut Handler<C>` as a side effect of placing,
so an `after` could register in front of the child but never hold the
child's dispatch.

The missing power already exists as `capture`
(`puri/src/handler.rs:160`), whose own doc names the whole vocabulary
— "call, wrap with before/after behavior, transform events for, or
drop" — and whose test `captured_children_dispatch_through_their_
wrapper` is `around` in miniature. The gap is that `capture` takes
`&mut P` at imperative placement time and has no `Node<P>`
combinator to ride, which is why it is used nowhere in progred.

Sketch:

```rust
pub fn around<P>(
    child: Node<P>,
    wrap: impl FnOnce(&mut P, Rect, &mut dyn FnMut(&mut P)) + 'static,
) -> Node<P>
```

Placement needs the FnOnce-through-FnMut dance (`let mut child =
Some(*child)`, `child.take()` inside the continuation). Then
`before`/`after` fall out, and `capture` composes inside the wrap to
give Roc's `map_frame`.

Do NOT add a `map_handler` helper alongside it. Roc added exactly
that (`map_handle`) and deleted it one commit later (`4ec7822`):
building a fresh handler from a function that closes over the old
one's dispatch is more direct. Destructure `Handler { pointer_down,
.. }` and rebuild.

What it buys at the composition root — `todo/TodoUi.roc:page!` states
the whole precedence stack in two lines:

```roc
with_focus = RoclayLayout.before(KeyboardFocus.widget({...}), page)  # tried last: the fallback
RoclayLayout.after(task_view.overlay!, with_focus)                   # tried first: the overlay wins
```

`run_frame` expresses the same thing imperatively by ordering
`place_top_left` calls (body, then graph pane, then popup card),
which works but is not composable.

Naming: once both halves exist, `decorate`/`decorate_after` reads
worse than `before`/`after`, which name when the callback runs.
17 call sites in `raw.rs`; mechanical. Open taste call — `decorate`
also carries "same extent, transparent wrapper", which Roclay
documents on `around` rather than encoding in the name.

## 2. `EventLoop` as a tested puri module

Roc: `puri/EventLoop.roc` (45 lines), `puri/tests/EventLoopTests.roc`
(50 lines).
Here: spread across `progred/src/main.rs:429` (the dispatch block),
`:1175` (`retain_dispatch`), `:1833` (`redraw`).

Batch of events → each gets a frame built from the state the previous
event produced → only the last one draws → an empty batch becomes one
timestamped `TimePassed`. This is the policy that took four
redesigns in two days here (recorded in `puri.md`'s History) and it
is currently tested nowhere.

The shell parts (winit, menus, the discard sheet) do not extract, but
the core is a pure function of `(events, state, build_frame)`.
Highest-value structural port.

Note the Roc module is shaped by RocRay's game loop — it consumes a
BATCH per rendered frame and always redraws, which is why an empty
batch has to synthesize `TimePassed`. Here the loop is
`request_redraw`-driven and events arrive one at a time, so what
transfers is the semantics and the test, not the batch signature.

## 3. The `Interact` vocabulary

Roc: `puri/Interact.roc`.
Here: `puri/src/interact.rs` exports exactly `clickable`.

`raw.rs` hand-rolls 11 `on_pointer_down` registrations, 10 of which
repeat `event.button == Some(PointerButton::Primary) &&
rect.contains(...)` verbatim (`rename_target:3229`,
`cursor_target:3326`, and friends). Roc's ladder —
`on_primary_pointer_down_where` / `on_primary_pointer_down` /
`on_primary_click` / `clickable` / `double_clickable` — collapses all
of them. Cheap, mechanical, immediately shrinks `raw.rs`.

## 4. `KeyboardFocus`

Roc: `puri/KeyboardFocus.roc` (69 lines), used via
`todo/TodoFocus.roc`.

Pure, non-rendering, owns no focus state, infers nothing from the
tree: the app supplies an ordered list of `Entry { focused, focus! }`
and the module handles Tab, Shift-Tab, unconsumed Escape, and
primary-click clearing.

`puri.md` promised exactly this ("pure helpers, ordered focusable
keys plus current → next/previous, advisory") and it was never built
— there is no `Tab` handling anywhere in progred.

Worth knowing how Roc arrived there. Its `Handler` originally carried
focus traversal INSIDE the monoid — `FocusTraversal { first, last,
next, previous, has_focus }` combining as handlers composed, i.e. a
tab order derived automatically from the placement tree. `799ab25`
deleted it in favor of an application-supplied order. That is the
same argument `MOTIVATION.md` makes about identity: an order inferred
from output structure makes harmless wrapping and refactoring change
behavior. Don't rebuild the derived version here.

## 5. Scroll viewport child-gating policy

Roc: `puri/ScrollView.roc`.
Here: `puri/src/scroll.rs` says a sub-viewport panel is "a later,
separate widget"; `puri.md` says it will gate children by composing
their captured handler.

Roc has shipped it, and the policy is the part worth stealing: gate
pointer-down and scroll by viewport containment, but leave MOVES AND
RELEASES UNBOUNDED so a drag begun inside can finish after the
pointer leaves the viewport.

Keep it as local widget policy. Roc briefly had it as a Handler API
function (`within_pointer_bounds`) and `4ec7822` moved it into
ScrollView under the heading "Keep handler API focused". The Handler
API there is now four functions — `default`, `plus`, `from_function`,
`dispatch!` — and nothing else.

Note Roc also threads a `clip_rect` beside `rect` in `Placement` and
hit-tests against it throughout (`Interact`, `EditableText`,
`Button`). `puri.md` rejected that as the retained-region wrongness
the handler redesign removed. Roc's experience is a counter-datapoint
— it appears not to have been regretted — so treat this as an open
question to re-decide when the sub-viewport arrives, not as settled.

This is also `Handler::on_scroll`'s first consumer. The channel
currently has ZERO registrations app-wide; `dispatch_scroll` at
`main.rs:506` always falls through to `graph_scroll` /
`scroll_document`. Unexercised, not dead — keep it.

## 6. `Interact.adjust_click_run`

Roc: `puri/Interact.roc`, state in `todo/Todo.roc`
(`pointer_click_offset`), applied in `TodoTaskRow.roc` via
`Layout.map_frame`.

Translates the remainder of a multi-click run into a newly presented
frame's frame of reference: subtract the transition clicks, and a raw
single click resets the run instead. Roc needs it because its label
editor mounts on physical click 2, so click 3 arrives as a triple and
should be the editor's double.

Not a bug here today — every editor mounts on click 1, so counts line
up, and `main.rs:2066` (`let count = if fresh { 1 } else {
click.count }`) covers the two cases we have. This is the general,
composable form of that ad-hoc line, and the "raw single click resets
the run" case is the part that is easy to get wrong. Rides on item 1.

## 7. `Drag` + `Drag.hysteresis` + `Reorder`

Roc: `puri/Drag.roc`, `puri/Reorder.roc`,
`puri-roclay/ReorderableList.roc`.

Layout-independent drag routing that retains no gesture state: the
caller decides when a drag is active and supplies the transitions.
`Reorder` is the transient preview (armed index → dragging with gap
index, grab offset, row size); the list itself never changes until
commit.

`Drag.hysteresis` is the standalone nugget: a handler that swallows
pointer moves within N px of an origin so a LAYOUT CHANGE UNDER A
HELD POINTER does not begin a selection drag in whatever replaced it.
No gesture state at all — just an origin and a threshold, composed in
front of the widget that would otherwise claim the move.

`graph_view.rs` already grew its own bespoke drag with click slop; a
structure editor will eventually want row reordering. Not urgent.

Take the DECOMPOSITION (source / motion / release / hysteresis as
four separate invisible widgets, gesture state owned by the caller),
not the signatures: `Drag.Begin(state) : state, Placement,
PointerButtonEvent => state` is state threading, and here it is
`Fn(&mut C, Rect, &PointerButtonEvent)`.

## 8. Time as input

Roc: `puri/Event.roc` — every event carries `timestamp_nanos`, and
`EventLoop` synthesizes one `TimePassed` for an empty batch.

This is the substrate the caret-blink item in `puri.md`'s adoption
backlog needs ("elapsed-ms state fits time-as-input"), and it is much
cheaper to decide before there is an animation than after.

## 9. Clipboard as a capability, not a direct call

The OBSERVATION, which is language-independent:
`puri/src/edit.rs:264-281` constructs
`clipboard_rs::ClipboardContext` inside the widget, three times in
one match, and `puri/Cargo.toml` depends on `clipboard-rs`. It is
the one place puri reaches the platform directly. It costs backend
independence (any Puri consumer inherits clipboard-rs), testing
(clipboard behavior cannot be exercised without a real system
clipboard, which is why none of it is), and duplication — the shell
already owns a richer path at `main.rs:1527-1612` with the
`com.progred.value` pasteboard type, so there are two
implementations with different capabilities.

The Roc SHAPE, which should NOT cross over: `Clipboard(state) : {
read! : state => { state, text }, write! : state, Str => state }`.
Threading the app state through a clipboard read to get text back in
a record is pure Roc pure-state plumbing. Do not add that.

Rust-shaped options, in increasing order of ceremony:

- Decline the clipboard chords in `edit.rs` entirely and let the
  shell own all of it. `main.rs:445` already runs `clipboard_key` as
  a fallback after the editor declines, so progred would simply get
  all four chords instead of the two it gets today — one
  implementation, one pasteboard policy, and puri loses a dependency
  instead of gaining a parameter. Cost: a non-progred Puri consumer
  gets a text editor with no clipboard until it writes its own.
- Put clipboard access on `EditCtx`, beside `fonts` and `layouts`.
  It is already the "what a dispatch needs from the caller" record,
  and the dispatch already has `&mut C`.
- A `&dyn Clipboard` parameter on `text_edit`. Most explicit, most
  ceremony, and probably not worth it.

Worth noting parley's own `vello_editor` — the source this was
transplanted from — also calls clipboard-rs directly, so the status
quo is a defensible default rather than an oversight. The reason to
revisit is the duplication with the shell, not purity for its own
sake.

## 10. Documentation

`MOTIVATION.md` in the Roc repo is largely the essay `puri.md`'s
Linebender-strategy section says to write once Progred runs on Puri.
"Identity is history, not output" and the two-eliminations framing —
explicit state kills cross-frame identity, placement continuations
kill within-frame output identity — are sharper than anything here,
where the docs are decision-log shaped.

Three implementations across three type systems is also a much
stronger claim than one.

---

# Part II — The simplification pass

The Roc source got a manual simplification review this one has not.
These are the shapes it found, each with what it would look like
here. Ordered by how likely there is something real to find.

## S1. Correlated fields collapse into a sum

`9586b76` replaced `scroll_offset : F32` plus `scroll_to_end : Bool`
with one type:

```roc
# AtEnd defers the concrete offset until placement, where the viewport and
# content sizes are known. Manual scrolling transitions to AtOffset.
Position : [AtOffset(Geometry.Scalar), AtEnd]
```

Two fields where one is a mode flag over the other is the smell; the
"which wins" question stops existing.

Candidate here: `model.scroll: f64` beside `App::revealed:
Option<(Path, Discriminant<Selection>)>`, whose only job is to make
`reveal_selection` fire once per selection change and not fight
manual scrolling — exactly `scroll_to_end`'s job. As a sum, "I want
that path visible" would be a state that manual scrolling transitions
out of, and the memo plus the equality check at `main.rs:1115` would
both disappear.

Honest caveat: `AtEnd` resolves from `content_size` alone, while
`Reveal(path)` needs one specific node's settled rect, which is not
available before placement — so it would still resolve against the
retained dispatch rather than during the pass. The memo goes; the
read-back doesn't.

## S2. Hover as a pass input rather than model state

Roc has no hover state at all. `main.roc` reads `host.mouse` each
frame, `TodoUi.ui!` threads `pointer_position` down, and each widget
computes `hovered` from its own settled rect.

Here it is `Model::hover`, written by move dispatch through
`Hooks::hover`, plus `App::hover_claimed`, `App::pointer`,
`App::pressed`, and `refresh_hover`, which replays a synthetic
`PointerUpdate` through the retained handler at every mint — and, at
`main.rs:1894`, can request a SECOND redraw because the frame just
drawn had the wrong answer.

Not a straight port: Roc's version has no innermost-wins and no
gap-crossing hysteresis, both of which this projection needs, and
which is why the claim protocol exists. The shape worth weighing is
making the hover resolve a pass OUTPUT (like `descends`,
`max_scroll`) computed from `pointer` as an input, so the resolve
pass always precedes the draw pass.

Cost is explicit: always two traversals per redraw, versus today's
one plus a rare corrective frame. Given the standing rule that the
first presented frame must be final, the trade may be worth it —
but it is a trade, not a free win.

## S3. Keep the core API small; push policy to the widget

Two deletions in the same spirit, both already noted above:
`within_pointer_bounds` out of `Handler` and into `ScrollView`
(`4ec7822`), and `FocusTraversal` out of `Handler` and into
`KeyboardFocus` (`799ab25`). The Roc `Handler` is now 33 lines.

Ours is 143 lines, of which roughly 120 is six channels × (field +
`on_*` + `dispatch_*`) plus their `Default`. That boilerplate is the
price of closed enums and is not obviously reducible — but it is
worth confirming nothing else has accreted there. `HasHandler` for
`Handler<C>` itself, for instance, exists only so `capture` can be
called on a bare handler in tests.

Do not read this as "collapse the channels." Roc's unified
`handle_event!` works because its event type is an OPEN tag union —
a widget names only the cases it understands and backends add
others. Rust has no open sums; a unified `enum Event` is closed, so
every widget matches every kind and every new kind touches the enum.
`puri.md` already recorded that rollback. Typed channels are the
closed-world form of the same idea.

## S4. Hoist a helper into the module that owns the concept

`c8a7c9c` moved `fit_label!` out of `Checkbox` and into
`Text.fit_with_ellipsis!`, where the ellipsis-fitting concept
actually lives.

Candidate sweep here: measurement, caret-index, and rect-fitting
helpers that live in `raw.rs` but belong in `puri::text` or
`puri::edit`. `caret_index` is the obvious first one to look at.

## S5. One match with patterns and guards, not nested match/if

`38bfded` and `c8a7c9c` both collapse this shape:

```roc
PointerDown({ button: Some(Primary), position, .. }) if contains(rect, position) => ...
Key({ state: KeyDown, key: Named(Enter), .. }) | Key({ state: KeyDown, key: Named(Space), .. }) if focused => ...
_ => Declined
```

replacing two intermediate handler functions plus an outer
dispatching match. Rust has the same patterns — destructuring, `|`
alternatives, `if` guards, and let-chains, which `main.rs` already
uses well. The handler closures in `raw.rs` are where to look.

## S6. Mutable accumulators — mostly not a Roc lesson

`38bfded` removed a `var $frame` that existed only to be reassigned
once, and Roc's remaining `var $x` loops (`Reorder.move`,
`Text.fit_with_ellipsis!`, `CaretMap`) are what code looks like in a
language still short on iterator combinators. Rust has the
combinators and `AGENTS.md` already carries the rule, with its own
escape hatch: "over mutable accumulators and loops WHERE IT DOESN'T
MAKE THINGS WORSE."

`raw.rs` has 83 `let mut` and `edit.rs` 27, but a count is a prompt
to look, not a finding — many are parley drivers and scene building,
where mutation is the honest expression. If a Rust simplification
pass happens it should be driven by the existing `AGENTS.md` rules
against real call sites, not by this file. Recorded so nobody treats
the count as a defect list.

## S7. Descriptions, not positional argument lists

Every Roc widget takes one named `Description` record.
`ROC_NOTES.md` gives the reason concretely: Button's "named content
record makes `focused` and `hovered` visible at call sites instead
of passing two easily transposed booleans."

`raw::project` takes thirteen positional parameters and its first
act is to build a `Cx` from them. Four are adjacent optionals —
`selection: Option<&Selection>`, `graph_node: Option<&Value>`,
`hover: Option<&Hover>`, `hover_node: Option<&Value>` — of which two
are the SAME type, so transposing them at the call site is silent.
`run_frame` carries `#[allow(clippy::too_many_arguments)]` for the
same reason.

The fix is half-built: `Cx` already exists and is exactly the
description. The question is whether the caller should construct it
rather than the callee, and whether the `Hooks`/`TextCtx`/`Node`
outputs belong beside it.

## S8. Focus and its transitions as one value — WITHDRAWN

`EditableText.Interaction := [Unfocused(Focus(state)),
Focused({ selection, change!, submit!, cancel!, clipboard })]` looks
like S1's shape applied to `text_edit(state, focused: bool, …,
with)`, where the bool and the callback that reaches the editor are
separable.

It isn't. `with` returning `None` while `focused` is true is a
DESIGNED state, not an incoherent one — `puri.md` puts it plainly:
"handlers must still decline on absent state rather than assume it —
the editor hook returns Option for exactly this," covering the
windows where mutation happens outside dispatch and the handler is
a frame stale. Collapsing the two into a sum would erase exactly the
distinction that decline depends on.

The Roc sum is also carrying the callbacks because Roc has no other
way to hand behavior to a widget. Kept here as a recorded dead end
so the shape isn't rediscovered and mistaken for a finding.

## S9. Recorder coverage where the geometry lives

Roc tests widgets by handing them a literal `Placement` and
asserting on the recorded commands plus the dispatch result, which
is what keeps `puri`'s tests independent of any layout engine.
`AGENTS.md` already states that policy here.

The gap is where the policy is not followed: `edit.rs` is puri's
largest widget at 1115 lines with ZERO `DrawList` assertions. Its
tests are behavioral (text, selection offsets), which is right as
far as it goes — but caret rectangles, selection rectangles, and
scroll-to-caret are geometry, and geometry is exactly what the
recorder is for. `delim.rs` is the same. Not a new idea; an
existing rule with a hole in it.

## S10. Confirmations, not work

Two Roc changes this repo already satisfies by another route; noting
them so the pass doesn't "fix" what is already right.

- `b86f381` DELETED task identity (`Task.id`, `next_id`) in favor of
  `TaskIndex` = position, on the reasoning that one-shot handlers
  capture it and list-changing transitions remap stored uses
  (`Reorder.move_index`). Progred already routes by `raw::Path` and
  reserves real identity for `Gid`, which is the domain-level case
  `MOTIVATION.md` explicitly allows.
- `807e8eb` funnelled every focus transition through
  `commit_active_edit` so leaving an editor always commits, rather
  than each call site remembering. Progred gets the same invariant
  from the single post-dispatch `write_through` sweep at
  `main.rs:524` — one door, arguably a better one.

---

# Not porting

- **Roclay.** The baseline box algebra is the smaller, sufficient
  model for a pretty-printer-shaped document; nothing in the Todo
  argues otherwise. Keep it as proof the placement interface COULD
  host a flex engine.
- **The monoidal `result` with `default`/`plus`.** That is Roc paying
  for the absence of higher-kinded types (`ROC_NOTES.md` says so
  plainly). The `&mut P` canvas trait with `DrawList` as one
  interpreter is strictly better and already generic.
- **The pure-state `Handler`** (`(state, event) → Handled(state) |
  Declined`). It buys one real property — a declined handler CANNOT
  smuggle a state change, which here is only the convention in
  `on_*`'s doc comment. Paying for it means the context becomes a
  cloneable value, and `C = App` holds GPU resources and parley
  caches. Not portable.
- **A unified event enum.** See S3.
- **Placement returning a value.** Roc's `Place(output) : Placement
  => output` means anything a subtree hands outward is a return
  composed with `plus`; ours becomes a mutable field on the context
  (`Frame<'a>` has four — `descends`, `max_scroll`, `max_scroll_x`,
  `popup` — each with a trait to reach it). Keep the mutable form: it
  is what makes `Canvas` allocation-free, and the popup genuinely
  must be placed at the root anyway (it escapes the scroll clip and
  flips above/below the anchor). Note this is where half of
  `around`'s job already went, which is why the missing halves have
  not bitten.
- **The `F32` scalar choice.** A prototype tradeoff Roc made to avoid
  repeating unnameable constraint groups; kurbo's `f64` is fine.

# Caveats carried over

`ROC_NOTES.md` documents `--opt=speed` builds hitting 10+ GB and
being OOM-killed, localized to generic tree passes over a
Puri-shaped `Frame`. That is a Roc specialization bug, not a design
verdict — but it is a datapoint that this architecture is
monomorphization-heavy by construction, and `Node<P>` over a generic
canvas and context monomorphizes here too. Worth watching if build
times creep.

One honest symmetry while reading Roc's `output`: neither
implementation can transform DRAWS in production. Under a direct
canvas the ink has already hit the screen when `place_inner!()`
returns; `placement_result` only carries information under a
recording interpreter. Roc's floating-row overlay
(`visual_only = Frame.from_placement_result(...)`) works because
dropping the HANDLER is the meaningful part. Placement-as-value buys
handler interception — which `capture` already gives us.
