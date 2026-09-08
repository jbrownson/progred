# Deferred work

These are open questions carried forward from the review, not tasks scheduled
for the next cleanup. The current implementation is documented in
[model.md](model.md), [projections.md](projections.md), and [puri.md](puri.md).

## Active insertion hidden by a specialized projection

A specialized projection can omit the structure where an insertion is pending.
The general interaction and presentation policy needs dedicated design work.
Do not hide a fallback policy in reusable widgets or infer it by inspecting a
render tree. This was explicitly set aside for another day.

The shared list/record combinators preserve immediate pending children. Compact
Grap lambdas, cases, binding clauses, and control/operator calls now decline when
they cannot show an active insertion. That fixes those specific projections;
it is not a general fallback policy for arbitrary facets such as line controls.

## Selection destinations and history

Plain selections now work with line controls' missing-state defaults, so paste,
deletion, and undo do not need editor-initialization hooks. Destination policies
may still merit customization: deletion chooses a surviving sibling or parent;
undo restores an edge location, not its prior caret or pending query. Preserving
those states is a separate UX decision, not necessary to make restored selections
usable. Never resurrect active drags or IME composition from history.

Line-editor conversion runs after every accepted event, including caret
movement; equal results already skip document writes. Avoiding unnecessary
conversion is a separate optimization. Projections must not rely on that
optimization to keep a missing value absent: a direct line control offers its
spelling for write-through. Lambda names instead use the completion picker,
which stages input until commit.

## Duplicate-definition inspection

Normal lookup selects the document definition, otherwise the first library
definition in load order. Duplicate definitions are tolerated, not an intended
composition mechanism. Later we may show an indicator and an inspector for the
other sources; no such UI is implemented now. Explicit source-qualified paths
remain available internally.

## Editor-authored libraries

Libraries now have ordinary description cells and unified definition tables.
Their native projection and completion contributions still use Rust callbacks;
there is not yet a GID convention and decoder for a complete editor-authored
library. Loading such a library should be an explicit editor operation, not
an evaluator FFI or implicit discovery of reserved cells. Keep configuration
and definitions reachable from root. The proposed descriptor lists cell references,
not a second record duplicating their definitions. Loading needs that descriptor
and a source that can resolve the listed cells; a document can provide that source
without becoming part of the library's semantic type. Authoring/loading UI remains
deferred until there is a compelling use case.

## Address lifetime

Paths and source traces already preserve definition source. A future identity
review should specify what survives list moves, deletion/undo, definition
replacement, and transient-root changes. A cell has meaning in a resolution
context; neither a path nor a cell-relative trace is a global address.
Keep occurrence identity, cell identity, definition source, and view identity
distinct while answering those questions.

## Completion ranking

Revisit how library-provided offers rank alongside built-in constructors,
cell references, and interpretations of the query. The universal list currently
groups library offers together; with an empty query, `new lambda` appears among
number literals because of library order. Providers cannot express that it
belongs with other constructors. Design that distinction through the completion
interface rather than special-casing lambda or particular numeric libraries.
This is deferred UX design, not a priority for the current cleanup.

## Reporting completion failures

Offer construction decides applicability; activating an offered completion
consumes the input. Its Grap continuation can still fail at runtime. For now,
unsuccessful preparation installs neither the insertion nor its staged
selection/annotation changes. Declining after an effect is now an evaluator
contract error and prints to stderr; other completion failures remain silent.

Later, design an editor-owned way to surface these failures, such as alerts or
notifications, using the failure details in GID values. Do not add that machinery
now. Explicit decline is distinct from ordinary absent results, which are
definitive values and retain their effects. The eventual reporting policy must
not assume every absent warrants an alert.

## General computation reuse

The canvas memo was removed because its validation observed less than evaluation
could use. Current canvas programs record once per visible frame; text shaping
is the cross-frame memo. Any future invalidation system must track actual
lookup results, including absence, ordered definition sets, and foreign
implementations. Settle the model before adding it. Do not reintroduce an
ad-hoc cache or event-specific relevance checks.

The [2026-09-04 tree profile](tree-profile-2026-09-04.md) records measurements
from that implementation; timings are historical observations, not guarantees.

## Hover traversal order

Revisit when the document/widget mix includes more expensive overlapping hover
targets, especially canvases covered by opaque popups or other content. Current
placement runs probes in painting order; foreground claims replace background
claims, but cannot avoid the work of background probes already run.

Compare this streaming pass with retaining probes and querying topmost-first,
where a direct hit or occlusion can skip probes underneath. Measure skipped
hit-test work against the extra allocations, retained captures, and traversal;
include ordinary source documents as well as overlap-heavy cases. Preserve
direct-over-extended claim priority, clipping, and nested-floater ordering, and
check identical hover, paint, and dispatch results. This is a deferred experiment,
not a decision to reverse traversal or add caching.

## Platform work

### Fidget constant fields on the GPU — upstream fix to adopt

Fidget 0.5.0's GPU `RenderShape` allocates its variable buffer from the compiled
variable count. A constant field therefore creates a zero-byte buffer and tries
to bind it as storage. Creating a zero-sized WebGPU buffer is allowed; binding
a zero-sized range is not. This is platform-independent validation, not a Metal
quirk; see [WebGPU buffer creation](https://gpuweb.github.io/gpuweb/#buffer-creation)
and [bind-group validation](https://gpuweb.github.io/gpuweb/#dom-gpudevice-createbindgroup).

Upstream checked on 2026-09-05 at main commit
[`94123ea`](https://github.com/mkeeter/fidget/blob/94123ea4a351fedf66df404c31f3fed0e1474b0e/fidget-wgpu/src/lib.rs#L280-L293):
this is already fixed by allocating at least four bytes. The change landed on
2026-08-25 in [PR #461](https://github.com/mkeeter/fidget/pull/461), commit
[`19a9049`](https://github.com/mkeeter/fidget/commit/19a904926a72a4e8509d02060bfef109fe37e488).
Searches of open/closed issues and PRs for constants, buffers, and bindings found
no separate matching bug report. The older [PR #318](https://github.com/mkeeter/fidget/pull/318)
fixes a different constant-evaluation problem. Do not file a duplicate report
for the allocation/binding defect already fixed in #461.

Progred remains pinned to 0.5.0. By explicit choice, there is no constant-field
workaround: the temporary CPU-routing special case was removed rather than
retained after its upstream fix. Rendering a zero-variable field on the GPU
can therefore still fail, including expressions compiled down to constants.
The ordinary CPU backend remains available when no GPU is available and on
the web; its tests do not verify GPU behavior.

At the next reviewed dependency update, verify the upstream fix is included
and test zero, positive, negative, and computed constant fields on the real GPU.
If validation still fails, prepare a minimal standalone Fidget reproducer and
capture the exact validation error, dependency versions, OS, and adapter/backend
for an upstream report. The original Progred crash did not preserve the GPU
validation message, so its precise cause remains inferred from this code path.
Metal itself is unavailable inside the build sandbox. Do not forbid constant
fields in the Fidget language or merely hide their completions.

### Additional ports

The iOS host remains a feasibility prototype. Further product work is deferred
while macOS/Linux mature. See [platform notes](platforms.md) for its current
build path and limitations. Older visionOS and language-extension ideas remain
uncommitted possibilities in the historical notes.
