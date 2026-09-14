# Toolpaths

The toolpath library is an experimental path-geometry layer, not a machine
program or a collision/clearance check. It adds no evaluator or editor-core
cases and requires no new dependencies.

## Generation and interpretation

Rust generators call `paths::Sink::start_at` and `line_to` with finite 3D
coordinates. A new start begins a separate path: it is neither a cutting link
nor a rapid move from the previous endpoint. A line requires a preceding start.
Calling generators in sequence composes their output. `MapPoints` adapts another
sink, so translation, reflection, surface mapping, and further adapters compose
without recording intermediate paths.

`Recording` is an optional initial representation: a native vector of start/line
commands, with replay into any sink. Counting or direct rendering consumers can
skip it. There is no GID command list and no opaque Rust program passed through
Grap. A preview records locally while preparing each frame, projects the points
to one vector path, and paints that path through the ordinary canvas interface.
Nothing is memoized across frames.

Grap uses ordinary calls to scoped `start at`, `line to`, and
`map points` functions. `do` supplies sequencing; lambdas supply reusable
generators. Emitters outside an installed output scope return `toolpath output
required`. `map points` takes a callable under `mapping` and an unevaluated
`expression`. For each emitted point, it calls the mapping with `x`, `y`, `z`;
the mapping returns an ordinary record with those fields (the `point` function
constructs one). Nested mappings run inside-out and restore the surrounding
mapping on exit, including on failure. Mapping functions are intended to
compute coordinates, not emit more paths.

The scoped output owns its mapping chain and sink. No mutable state is held by
the library between evaluations. Each emitted sample consumes evaluator fuel,
including samples generated inside Rust. Invalid commands mark the scoped
result failed even if a surrounding `do` ignores their returned absent. A
consumer needing atomic results must stage its sink and discard it on failure;
the preview does. Native generators propagate their sink's errors immediately.

## First example

The example's Grap `diagonal passes` and `sample pass` functions follow
`ToolPathHelpers.DiagonalUVs` in
`rhino-cube-plugin/src/models/Paths/ToolPath.cs`. It distributes nondegenerate
interior diagonals across the unit square and samples each including both
endpoints. Its maximum sample spacing is in **UV coordinates**, before mapping;
it is not a world-space tolerance, stepover, or scallop-height guarantee.
The row loop and point loop use the ordinary `iterate` function, emitting
`start at` and `line to` calls without collecting intermediate lists. Sampling
policy is document code, not a toolpath FFI. General `min`, `max`, `ceil`,
`hypot`, and `is finite` operations belong to the f64 library. The example
rejects nonfinite or fractional row counts and nonpositive/nonfinite spacing;
an excessive sampling request is bounded by evaluator fuel.

The example maps two crossing sweeps to the top quadratic patch used by the
Rhino cube (size 1, chamfer 0.1, center-control-point displacement 0.5):

```
x = 0.8(u − 0.5)
y = 0.8(v − 0.5)
z = 0.5 − 2u(1 − u)v(1 − v)
```

This corresponds to a face-center depression of 0.125. The second sweep reflects
the unit-square y coordinate before the same surface mapping. Unlike Rhino's
finishing program, this example does not reverse row order for that second
sweep, compensate for the ball radius/tool orientation, or construct linking
curves. Those are separate future combinators. The first fixture covers one
face only and retains the original model units without assigning a machine unit.

`preview paths` is an opt-in viewport function with `value` (a callable program),
`width`, `height`, and optional `fuel` (otherwise Grap's default). It returns an
ordinary preview declaration containing that program and these parameters. The
native widget runs the program with this explicit budget. The example assigns
100,000 fuel for its 42 passes. The preview fits a fixed isometric projection
inside the assigned pane, with a 16-logical-unit margin, reduced in tiny panes.
All paths are green; there is no inferred cut/link classification.

Cutter playback, orientation, radius compensation, explicit links, stock
removal, and machine/postprocessor output remain separate next steps. In
particular, the existing Fidget cube field is not an exact signed distance;
subtracting a cutter radius from it would not implement a geometric offset.
