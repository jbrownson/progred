# Rhino-derived fidget cube

The editable model is [fidget-cube.gid](../examples/fidget-cube.gid), available
as Examples → Fidget cube (Command+8 / Ctrl+8). Its source is a no-argument Grap
function; the body constructs ordinary Fidget fields through existing library
functions. There is no cube primitive in the evaluator or Fidget adapter.

## Reference

The reference is the owner's `rhino-cube-plugin` checkout at commit `1fe8fbc`
(2025-06-08), especially:

- `src/models/ShinyCube.cs`: size, chamfer size, and depth.
- `src/ShinyCubeRhino.cs`: six quadratic surface patches and twelve planar chamfers.
- `src/commands/CreateShinyCubeCommand.cs`: defaults `1`, `0.1`, and `0.5`.

The older `rhino-cube` checkout has the same underlying surface construction,
but the plugin contains the later model and machining work. This port concerns
the part's shape only; it does not port tools, toolpaths, machine kinematics,
or G-code. Neither external checkout is needed to load the example.

Rhino's [`NurbsSurface.CreateFromPoints`](https://developer.rhino3d.com/api/RhinoCommon/html/M_Rhino_Geometry_NurbsSurface_CreateFromPoints.htm)
uses control points, not interpolation points. With three points and degree two
in each direction, each face is a tensor-product quadratic Bézier patch. Only
its middle control point moves inward. At the patch center its weight is
`(1/2) × (1/2) = 1/4`, so a control depth of `0.5` produces a surface depression
of `0.125`. The example deliberately preserves this distinction.

## Field construction

Let `s` be size, `c` chamfer, and `d` control-point depth. The bindings compute:

```text
h = s / 2
a = h - c
D = d / 4
L = s - c
q(t) = max(0, 1 - (t / a)²)
```

One opposing face pair is:

```text
face(n, u, v) = |n| - h + D q(u) q(v)
```

On the top face this gives `z = h - D(1 - (x/a)²)(1 - (y/a)²)` within
the square `[-a,a]²`: the Rhino quadratic patch, not a spherical approximation.
Clamping each factor to zero extends the field outside that square without
introducing another depression beyond the face. Chamfer constraints trim that
extension; it does not add exposed flat strips to the part.

An opposing set of chamfer planes is:

```text
chamfer(u, v) = |u| + |v| - L
```

The full field is the maximum of the three face-pair fields (one per normal
axis) and the three chamfer-pair fields (`xy`, `xz`, `yz`). Negative is inside.
These give six concave faces and twelve hexagonal chamfers. Three chamfers meet
at each corner `(±(h-c/2), ±(h-c/2), ±(h-c/2))`; there is no additional triangular
corner face or rounding.

This is an implicit field with the intended boundary, **not an exact signed
distance function**. Fidget can render it as a field; its numeric magnitude
must not be mistaken for distance when building offsets or cutter clearance.

## Units, editing, and checks

The source keeps the Rhino program's numbers and model units. There is no unit
type or conversion in this example. The viewport uses explicit −0.6…0.6 bounds
and receives the pane's actual width and height; those viewing bounds do not
rescale the part. Larger dimensional edits may require zooming out or changing
the bounds.

The ordinary geometric regime requires `s > 0`, `0 ≤ c < s/2`, and a depth
small enough that neighboring face patches do not cross. The example does not
constrain scrubbing or implement a general CAD parameter-validity system.
Degenerate dimensions and excessive depth are not certified solids.

Tests evaluate the actual Grap example, compare sampled boundaries against an
independent evaluation of Rhino's nine-control-point patches on all six faces,
check the twelve chamfers and their corner joins, and vary the dimensions.
The frame tests separately send the generated field through the real Fidget
preview and check that it produces a visible surface. The text fixture also
round-trips and has no orphaned definitions.
