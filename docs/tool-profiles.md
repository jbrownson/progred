# Tool profiles

Tool geometry is ordinary GID data, lowered to native `Tool` / `Section` /
`Segment` values. The toolpath library owns the convention and its projection.
The axis origin is the tool tip; positive axial coordinates point toward the
spindle. Example dimensions are inches, but the representation itself has no
unit system.

A tool contains a `tool` list of sections. It describes geometry only, with no
approximation tolerance or rendering-quality setting.
Each section is `{cutting: [moves...]}` or `{non-cutting: [moves...]}`.
The two tags are distinct library identities, not boolean values; a section
must have exactly one of them. The tool starts implicitly at radius zero,
axial zero. Each subsequent section continues at the preceding section's final
point: changing cutting/non-cutting status does not move or reset the contour.
Moves use absolute `radius` and `axial` f64 coordinates; an omitted coordinate
keeps its preceding value, including across section boundaries. A move must
specify at least one.

Without an arc field, a move is straight: radius-only makes a shoulder,
axial-only extends the current radius, and both can describe a taper. An
optional `convex arc` or `concave arc` field specifies a positive curvature
radius, not the endpoint's distance from the tool axis. The bend selects the
minor arc and its center; both bend fields together are invalid. Convex is
counterclockwise in the radius-right/axial-up profile plane, concave clockwise,
unrelated to spindle rotation. No center or signed-radius convention is stored.
All records permit unrelated metadata.

For example, a 1/8-inch bull mill with a 0.010-inch corner radius and a
0.22-inch cutting length is (numbers shown in readable shorthand):

```text
{tool: [{cutting: [
  {radius: 0.0525},
  {convex arc: 0.010, axial: 0.010, radius: 0.0625},
  {axial: 0.22}
]}]}
```

Profiles are filled to the axis, capped at their ends, then revolved; no final
return to the axis is needed. Coordinates must be finite and nonnegative;
axial travel cannot go backward, while radii may grow or shrink. Arc validation
checks that the radius spans the chord, the whole arc advances axially, and
the whole arc stays outside the negative-radius half-plane. Tangency to adjacent
segments is not implicit. Rendering tolerance does not change validity.

Lowering removes starting/ending radial caps, coalesces consecutive radial
moves, and splits connected bands at zero-radius axial travel. Travel on the
axis contributes no material, allowing a non-cutting shank to start above the
tip without creating a cone beneath it. Sections stay in tip-to-spindle order;
the axial coordinate cannot go backward at a kind boundary either. Separate
sections of the same kind require a positive axial gap, rather
than representing touching or overlapping independently capped solids. Cutting
and non-cutting sections can meet. This is not yet a representation of hollow
sections or arbitrary closed outlines that turn back axially. Invalid profiles
decline the custom projection and remain visible as ordinary data; simulation
rejects them.

Profile validation and outline sampling use f64 geometry, without imposing a
rendering backend's numeric limits. Fidget sweep construction checks its own
f32 conversions and arithmetic, including segment heights and gaps between
cutting bands that collapse at that precision. Such a sweep can fail while the
tool definition remains valid and its 2D profile remains available. The mesh
renderer likewise checks the vertices it produces, independently of Fidget's
squared-radius limits.

`ball mill`, `square mill`, and `bull mill` construct that same data, taking
`tool diameter` and `tool length`; bull additionally takes
`corner radius`. Native constructors have the same semantics. Composition
is an ordered sequence of cutting/non-cutting runs, not independent profiles
that each restart at the origin or a separate composite-tool variant.
Non-cutting cylindrical and tapered shanks use ordinary endpoint moves; a
radius-only move can join a narrower neck to a wider shank. Constructors emit
the same compact format, omitting redundant positioning at connected boundaries.
An intentional axial gap requires going to radius zero, advancing axially, then
setting the next radius; an axial move at a nonzero radius makes material.

## Interpretation

Mesh and implicit display consume the profiles, with cutting sections orange
and non-cutting sections gray. The custom 2D projection shows a mirrored profile
beside its ordinary editable data. Constructor results are normal values, so a
Grap `evaluate` projection can show them too. Clicking the picture uses ordinary
selection: the stored tool, or the owning expression for an evaluated result.
The picture computes bounds from the line/arc geometry itself, including arc
extrema, then samples to a quarter logical display unit at its fitted size.
It does not depend on playback accuracy.

Playback has its own required, finite, positive `profile tolerance`, in the
same units as the tool geometry. It supplies this to stock subtraction and the
mesh/implicit tool display. Changing it changes the computation inputs, not
the tool definition. The example keeps the prior value, `0.00025` inches, now
in the playback record. Native `Tool::sweep` and `Stock::cut` similarly take
tolerance as an explicit operation argument.

Stock subtraction sweeps **only cutting sections**, independently, then unions
their swept solids. No convex hull fills the space between disconnected bands.
Linear radius/axial segments have exact continuous fixed-axis sweeps, including
motion along the spindle axis. Circular arcs are chord-subdivided using the
playback's profile tolerance, independently of meshing and raster resolution.
This approximates the cutter profile, not the motion between path samples.
The lowering rejects excessively small tolerances rather than silently limiting
their accuracy (4096 chords per arc, 8192 outline points per section).
Geometric validation is separate from sampling: an impractically fine request
can fail without making a valid tool definition invalid or hiding its picture.

The hemisphere-plus-cylinder profile has an exact optimized lowering to the
existing ball-end kernel. It recognizes geometry, not a constructor identity
or name, and is derived from the same profile data. This avoids greatly growing
the existing cube's Fidget expression. Continuous-radius profile pieces have
only outer end caps, not a cap at every chord boundary. At a shoulder the
lowerer joins the capped pieces with a contained radial tent: zero radius at
each neighboring chord's far endpoint, rising to the smaller shoulder radius
at the join. Linear interpolation keeps this connector inside both neighboring
bands. Its sweep therefore adds no material outside the intended sweep, while
making the shared cap disk strictly interior. The exposed annulus remains a
real shoulder surface. There are no epsilon offsets or guessed overlaps.

The mesh tool is a direct surface of revolution (currently 12 angular sides),
not a mesh extracted from its implicit field. Its axial arc samples use the
playback tolerance; angular tessellation is display quality, not cut geometry.

## Current integration and next steps

Tool selection is a scoped `with tool` program combinator, not a playback
setting. Nested groups retain their tools through recording, replay, cursor
display, and subtraction from shared stock. The example's ball-tool
constructor references the same diameter cell as contact compensation. Its
`ball tip` mapping first computes the ball center from the surface normal, then
rotates and subtracts radius times the spindle-facing axis. The square-tool
definition is a Grap quote, sharing editable diameter and cutting-length cells
with side-contour compensation. Its tapered neck and explicit shoulder to a
wider non-cutting shank remain ordinary profile data. An `evaluate` entry shows
the resulting profile beside the source definition.

The example's sibling square-tool groups now make side-contour chamfer cuts.
Op 1 owns the four top and four vertical edges; Op 2 owns the four bottom edges.
Next: add crosswise end passes, stepping along the chamfer length, as another
Grap strategy. These simulate
the ideal tool envelope, not microscopic tooth marks. Fixture clearance, safe
links, holders/collision checks, and machine/G-code output remain separate work.
