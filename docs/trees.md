# Final-encoded trees

The tree library separates building a hierarchy from choosing its representation.
A Grap program emits groups and leaf values through scoped native functions:

- `tree group {children: ...}` opens a group. A literal `children` list runs its
  expressions in order; nested literal lists open nested groups. A non-list
  expression runs normally, so it can iterate, branch, or call another builder.
- `tree leaf {value: expression}` evaluates and emits one value. The source link
  names the emitting `tree leaf` call, not its argument or evaluated result.
- `map tree leaves {mapping: callable, children: ...}` wraps leaf emission in its
  children without adding a group. The mapper receives `value`. Nested mappings
  compose inside-out and retain the original leaf's source.
- `collect tree {tree program: callable}` runs a zero-argument builder and returns
  an ordinary nested list. It requires exactly one root, which can be a leaf.

Rust consumers supply `tree::Sink` (`begin_group`, `end_group`, `leaf`) to
`tree::interpret`. They can process emissions directly without retaining a tree.
The supplied collector is one interpreter, not the meaning of the interface.
There is no new evaluator construct, GID atom, or opaque native value.

Explicit groups likewise name the emitting `tree group` call. Nested literal
lists that implicitly open groups name those lists, since they have no call site.
Attribution is optional
when no stored origin survives. It is never reconstructed by inspecting closures
or searching for equal values. It describes source code, not the full execution
environment or one invocation of repeated code.

Emissions are evaluation-local effects. An absent stops the current child
sequence; group and mapping scopes close on normal return, absent, and evaluator
halt. Ordinary absents retain earlier emissions if an enclosing Grap expression
recovers, as with other Grap effects. An unsuccessful top-level collection returns
its absent and exposes no partial tree. Calls to emission functions outside an
interpreter return `tree output required`.

## Editor collection

The native collector returns ordinary items alongside an explicit native hierarchy.
Each node retains its emission source and distinguishes a leaf from a group,
including a list-valued leaf or an empty group. Group children use the same list
positions as their returned items. Plain `collect tree` returns only those items,
deliberately forgetting that distinction; it has the same meaning in every scope.

The [controls](controls.md) library's `tree program cursor` consumes an emitting
program directly. It uses the native hierarchy to build the sliders and captures
that hierarchy's sources in their decorations. It returns ordinary items, range,
and playback position to Grap. No allocation-identity lookup reconnects returned
data with native state. The supplied key addresses the existing per-view/location
memo root and control state. Dependencies include observed
cell definitions and missing definitions. The complete retained result contains
all emission effects, so recording explicitly permits memo reuse; halts and
untracked calls still prevent reuse.

The CAM example emits its hierarchy this way, collecting it once within the cursor
for playback, range controls, and the preview. Mapping/rotation/tool wrappers transform emitted
leaf programs rather than recursively rebuilding a nested list. Knurl lines and
chamfer strokes emit from `iterate`, without first constructing an unfold list.
This preserves the existing generated-position limitation: regeneration assigns
positions by emission order, not by persistent identity of loop iterations.

The migration's release-build comparison on 2026-09-18 checked the complete
hierarchy and all 6,588 tool segments against the previous example. Uncached
construction measured roughly 7 ms before and 9 ms with source-linked emission
(three runs each). This is a provenance/composition change, not a speedup.
Tests separately verify that resize reuses both the exact collected tree and the
downstream path recording. The real CAM controls rebuilt in about 0.23–0.26 ms
after that collection, including source-link decorations (release build, three
pane widths; not a whole-frame timing). `profile_program_tree_construction` remains an ignored
manual benchmark for construction and reuse.
