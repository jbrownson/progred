# Runtime values through the editor

Moved from [Grap and projections](../projections.md#equivalent-host-representations)
on 2026-10-19. It records, consumer by consumer, how Progred came to carry
Grap's runtime values instead of materializing GID, as that document described
the migration then. The rule it arrived at stays in that section.

Progred now carries runtime values through evaluation results,
presentation, controls, layout descriptions, drawing programs, and retained
slider/event callbacks. Descending into a returned container retains its runtime
children. Existing data-oriented partial projections use an explicit adapter;
it borrows stored GID directly or lazily materializes a shared GID view. The
original runtime value remains available to runtime-aware partials and children.
Copying a result and writing document/UI data are GID boundaries.

Fold classification, collapsed-container display, and source highlighting inspect
runtime atoms/container shape directly rather than asking for a complete GID
view. Text, blob, and color partials likewise inspect only their own facets. A collapsed
value's picking handler retains the runtime value and materializes it only when
invoked. Empty partials use the runtime interface so declining without inspecting
anything does not itself request conversion. Expanded structural lists/records
inspect positions/labels directly and descend into retained runtime children;
stored list positions remain unchanged. Control-form partials and shallow Grap
reference/declaration helpers also inspect runtime inputs directly. General Grap
call/lambda/value/FFI projections, evaluate-form recognition, numeric facets, and
arithmetic notation now do likewise. Call layout inspects labels rather than
argument values; parameter discovery reads native closure parameters directly,
without materializing code or captures. Both `evaluate` and presentation's
`render` pass runtime syntax into evaluation, including memoized evaluation.
The memo input uses the same conservative runtime equality as results, so
closures with different origins cannot silently reuse one another. Those
origins now differ in their GID representations too. Domain-specific and other
legacy partials still use the GID adapter.
Opening the writable color picker or
starting a numeric scrub requests GID for document-editing callbacks; simply
recognizing or displaying those facets does not materialize the enclosing record.

Scoped layout programs retain runtime results through nested child collection,
absence propagation, and the final output check. An absent's detail fields may
themselves contain retained callbacks. The `drawing` constructor wraps its
runtime configuration unchanged; layout picking retains its payload until
activation writes it into editor state. Border-projection composition captures
its runtime callable in the wrapper's lexical environment and retains the
projected result inside its border/`at` description. Declaration projection also
passes its callable and argument as runtime values. Neither boundary needs to
materialize callback code or captures. Narrow path/paint readers still use GID
adapters.

Toolpath execution retains runtime programs through nested `sequence paths`,
point/axis mapper scopes, and tool-scope results. Mapper inputs and point results
use runtime numeric fields; absent details retain runtime children too. The 2D
preview constructor and projection pass the program through without materializing
it. Tool-profile parsing remains an explicit GID boundary. Tree collection,
mapping, memo inputs/results, and the tree-program cursor's control/view handoff
also retain runtime values. Tree memo comparisons include callable code/capture
identity and the separate source-linked hierarchy, not just serialized GID.
The 3D preview constructors, projections, and recording memo retain runtime
programs and results as well. Model, playback, color, and tool-profile decoders
still use GID views of their own data. Recorded failures adapt to GID at the
existing render-outcome boundary; worker requests contain geometry, never
runtime closures. Fidget and cutter projections check their identifying fields
before requesting a GID view, so unrelated executable containers aren't
materialized merely to decline a projection.
Widget preparation also receives the runtime value, not an eagerly materialized
GID copy. A selectable widget converts only when the user actually picks its
value into the editable document.

Memo result comparison still conservatively includes native closure code/capture
identity. GID equality now includes the explicit origin metadata instead of
silently discarding it.
