# Selection-focused neighbor callbacks — experiment

Status: integrated into production after the initial test-only experiment.
The former graph implementation is checkpointed in `bafda915`. Current contracts
are in `model.md` and `puri.md`; this report records the experiment and migration.

## Question

Can projections supply navigation with frame-construction callbacks instead of
building and resolving a graph containing every navigation stop?

The production implementation lives in
`progred/src/display/widget/navigation.rs`, with regression fixtures in
`progred/src/projection/tests/frame/neighbor.rs`. Their projection uses ordinary
list descent, row/column alternatives, placement wrappers, and native text
editors. There is no alternative layout interpreter or application mode flag.

## Interface and construction

The small underlying interface is:

```rust
type Receive<T> = Box<dyn FnOnce(Option<T>)>;
type Provider<T> = Rc<dyn Fn(Receive<T>)>;
struct Neighbors<T>([Provider<T>; 4]);
```

A selected occurrence requests four neighbors. A provider either supplies an
already-known target immediately or retains the receiver until its neighbor is
placed. These are construction callbacks, not event handlers or asynchronous
jobs. `None` is a completed answer meaning there is no neighbor.

Each subtree exposes four entry targets, one per arrival direction. That small
summary is useful for a sequence with no selectable container: forward entry
and backward entry may differ. A selectable container exposes itself in every
direction. It does not expose whichever leaf happened to place last.

`Sequence::begin_child` supplies the previous entry for backward requests and
collects forward requests. `end_child` answers requests from **earlier** children
with the current child's entry, then keeps the current child's forward requests.
Children without an entry do not consume a pending request. `finish` forwards
remaining requests to the parent. The root answers `None`.

A container supplies itself to children as their outside Left/Up neighbor;
its own Right/Down requests use the children's entry if present. This preserves
the checkpoint's policy for the sample, without claiming it is the final UX.

Only the selected occurrence requests outgoing navigation. Receivers are
one-shot. A facet and its whole-value wrapper can contribute the same occurrence;
the inner contribution owns its requests, rather than registering them twice.

The initial prototype stored four answers and used a placement-local adapter.
Production instead threads an explicit construction capability through
`HoverPass` and `HoverContext`. Each answer emits an ordinary `Event::Navigate`
handler capturing its destination. No graph or four-destination table survives
in the installed frame. Construction checks that all four requests were answered
and that no deferred receiver escaped. Each view has an independent construction
scope, so equal occurrence paths in different panes do not cross-connect.

## Verification and limits

Tests exercise immediate and deferred answers, empty children, nested boundary
forwarding, root completion, whole-container selection, and horizontal versus
vertical chosen layout alternatives. Every occurrence in the sample is compared
with the checkpoint graph's destinations. Discarded alternatives cannot answer
requests: registration happens during settled placement, not measurement.

Real text handlers get first refusal. Navigation selects with the ordinary
selection operation and no LineEdit-specific caret payload. A sequence of
text-editing and navigation events rebuilds the placed frame between events
without painting. Production `EditorRunner::update_frame` already installs a
successor dispatch synchronously after an accepted key; the prototype does not
replace that shell path.

The `jump` test navigates between two occurrences of the same source value and
then types through the destination's existing editing scope. No editing-route
or conject changes are part of this experiment.

The migration also covers shared containers used by Cmd+9, view isolation,
initial selection, and the existing reveal/scroll path. The root sends raw keys
first, then sends an ordinary semantic navigation event for an unhandled bare
arrow. Initial selection is another ordinary handler. Browser interaction timing
and subjective navigation policy still need manual use. Hysteresis and
navigation-history commands remain separate future work.

## Measurement

The initial ignored `neighbor_profile_real_projection_builds` test alternated callback
and graph construction on the same document: 20 lists of 20 text widgets, one
selected occurrence, five warmup pairs and forty measured pairs. It includes
projection, measurement, placement, hover binding, and graph resolution, but
not painting or GPU work. Stock line widgets still contribute their existing
graph stops in the callback case; this is not a complete graph-free application
benchmark. Both cases use the same line widgets and layout alternatives.

Two consecutive release runs on the development machine:

| Construction | Median, run 1 | Median, run 2 | p95, run 2 |
| --- | ---: | ---: | ---: |
| Neighbor callbacks | 0.763 ms | 0.759 ms | 0.819 ms |
| Navigation graph | 0.992 ms | 0.986 ms | 1.021 ms |

That is about 23% less time in this small headless construction fixture, not a
claim about whole-application responsiveness. The full Progred library suite
passed: 905 tests, 54 intentionally ignored, including the five new regressions
and the new opt-in benchmark.

That comparison fixture was removed with the graph backend during integration;
these are measurements of the prototype, not a fresh production benchmark.
Run the integrated regression fixtures with the repository sandbox:

```sh
./tools/sandbox-cargo test -p progred --lib --release neighbor -- --test-threads=1
```

The prototype allocates providers even for unselected siblings and still visits
all placed children. Selection-focused output is not selection-only projection.
Avoid describing it as allocation-free or a proven application speedup.

## Integration

Shared containers and leaf targets now use the callback implementation; the old
graph backend and LineEdit-specific arrival policy are removed. Targets capture
ordinary selection plus the existing editing scope. Defaults belong to the
destination widget, not navigation. No runtime backend switch was added.

`Examples → Keyboard Navigation` supplies nested and unequal lists, repeated
outline sections, shared cells, named numbers, missing locations, and computed
read-only results. It is a manual design fixture, not a claim that the current
container-entry policy is final. Geometry-based tie-breaking, history, Tab, and
example-menu reorganization are outside this migration.

Integration verification: Progred's release library suite passed (904 tests,
53 intentionally ignored), as did Puri's release library suite (70 tests,
one ignored), the native workspace check, and the threaded WebAssembly check.
The application was not launched for this work; the new example and Cmd+9 still
need hands-on review of the navigation policy.

## Subsequent policy trial

Targets now receive an optional arrival direction. Line widgets place the caret
at the start for Right and the end for Left; unspecified and vertical arrivals
keep the ordinary default. Navigation itself still knows nothing about text.

Expanded cells use `nav_inline`: horizontal entry exposes
their first/last content targets, vertical entry exposes the whole value.
Horizontal traversal does not return to the enclosing value. Vertical lists opt into `reading_order`,
which connects both horizontal and vertical sibling neighbors using the same
deferred-provider mechanism. Other vertical compositions remain vertical-only.
The actual example regression walks through `first`, `45`, and the next cell's
name in both directions, without changing document data. There is no hidden
delimiter-position state or navigation history.

Bracketed lists subsequently switched to `nav_container`: arrivals stop on
the list itself before horizontal movement enters its contents. Contents still
exit directly. The regression walks `E → [F, G] → F → G → H`, and the mirrored
Left traversal, using raw keys and the real text widgets. Empty lists stop once
and then continue. This separates entry exposure from boundary exit without
adding selection tags or history; cell traversal remains unchanged.

Cells then adopted the same `nav_container` policy: horizontal arrivals select
the reference before entering its contents, including for empty cells. This
uses the same combinator as lists; exits and vertical sibling movement remain
unchanged.

Outline sections now connect headings and visible bodies with `reading_order`,
so horizontal movement does not skip a body that backward entry can reach.
Unclaimed arrows normalize to navigation regardless of modifiers; text editing
and explicit shortcuts retain first refusal. The release Progred library suite
passes 914 tests, with 53 intentionally ignored.

## Next policy trial — not implemented

The latest discussion favors a single whole-container stop at the start of its
flow: `previous ↔ whole ↔ first child ↔ … ↔ last child ↔ next`. Backward entry
would reach the last child, and backing out of the first child would select the
whole. This replaces the current whole-on-arrival-from-either-side behavior.
Bracketed and unbracketed lists should share the policy. No edge-position state
or navigation history is needed for this proposed traversal.
