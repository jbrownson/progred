# Examples

Open these files normally to edit and save them. The Examples menu opens a fresh
copy instead; its shortcuts are Command+1…9 on macOS and Ctrl+1…9 in the drawn menu.

| Shortcut | Document | Purpose |
| --- | --- | --- |
| 1 | `sample.gid` | Basic data and projections |
| 2 | `grap-demo.gid` | Grap evaluation |
| 3 | `iop-tree.gid` | Editable tree drawing, inspired by Inventing on Principle |
| 4 | `fidget.gid` | Blue cutaway sphere and a separate gold sphere in one colored scene |
| 5 | `fidget-torus.gid` | Smooth torus: a small arithmetic field |
| 6 | `fidget-tanglecube.gid` | Polynomial surface with several handles |
| 7 | `fidget-gyroid.gid` | Dense trigonometric lattice clipped to a sphere |
| 8 | `fidget-cube.gid` | Rhino-derived fidget cube: concave quadratic faces and planar chamfers |
| 9 | `toolpaths.gid` | Two-operation CAM playback, progressive stock rendering, and tool profiles |

The torus, tanglecube, and gyroid documents contain literal Fidget data, not Rust geometry
primitives or Grap programs. Each has an editable source cell and one left-side
viewport referring to that cell. The viewport fills its pane; drag to orbit and
scroll to zoom. Separate documents make it possible to compare one render at a
time at the same pane size. Complexity here describes the field and surface,
not a guarantee of increasing frame times at every camera angle.

## Fields

Inside means negative; the boundary is zero. These formulas describe the data
trees in the fixtures, not a second implementation:

- **Torus:** `(sqrt(x² + y²) - 45)² + z² - 15²`. The two radii are 45 and 15.
- **Tanglecube:** `a⁴ - 5a² + b⁴ - 5b² + c⁴ - 5c² + 11.8`, with
  `(a, b, c) = (x, y, z) / 25`. This is the standard
  [tanglecube polynomial](https://www-sop.inria.fr/galaad/surface/).
- **Gyroid sphere:** `max(abs(sin(a)cos(b) + sin(b)cos(c) + sin(c)cos(a)) - 0.2,
  sqrt(a² + b² + c²) - 25)`, with `(a, b, c) = 0.4(x, y, z)`. This follows
  [Matt Keeter's Fidget gyroid-sphere example](https://github.com/mkeeter/fidget/blob/main/models/gyroid-sphere.rhai),
  rescaled to fit our default preview bounds. The source is part of Fidget,
  copyright Matthew Keeter, licensed under
  [MPL-2.0](https://www.mozilla.org/en-US/MPL/2.0/).

The preview bounds are currently −80…80 on each axis. All three shapes fit
inside them. Sine and cosine are ordinary Fidget-library operators; Grap can
also call their constructors when generating these same data structures.

## Fidget cube

`fidget-cube.gid` contains a no-argument Grap function which constructs ordinary
Fidget arithmetic. Its `where` bindings expose the Rhino defaults: size `1`,
chamfer `0.1`, and control-point depth `0.5`. The resulting face-center depression
is `0.125`, not `0.5`. Edit or scrub those constants in the source.

The left viewport calls the function and wraps its result in a cyan scene object
for `preview mesh` at mesh depth 5. The cube function itself still returns an ordinary field.
Its explicit bounds are −0.6…0.6, in the same model units; no geometry scaling
or cube-specific Rust primitive is involved. Orbit and zoom work normally, with
fresh CPU meshing on every frame and GPU triangle drawing on native platforms.
Expand `panes` to change `mesh depth` or use `preview 3d` for comparison.
See [the mesh viewport](../docs/fidget-mesh.md) for parameters and limitations.
See [the geometry derivation](../docs/fidget-cube.md) for correspondence to the
Rhino surfaces, parameter limitations, and what is not yet a CAM model.

Colors are editable RGB values in the documents. The small Fidget example uses
Grap quote/unquote to combine two fields in `{scene: [{field, color}, ...]}`;
they retain separate colors while sharing depth testing and lighting. This
first color interface is opaque only, not a transparency or texture system.

See [frame performance checks](../docs/performance.md) for repeatable orbit
measurements using the actual example documents.

## Toolpaths

`toolpaths.gid` uses Grap to generate two diagonal sweeps per face and map their points.
Both the row loop and the sampling loop are editable example functions; only
point emission and the generic mapping scope are native toolpath operations.
Separate callable programs define `Op 1 · top and four sides` and `Op 2 · bottom`;
`Preview · Op 1 + Op 2` plays both, with Op 2 occupying the last sixth of the
slider. They share part coordinates for preview but are intended for separate
machine programs, not a linking move or an automatically planned stock flip.
The left viewport combines retained mesh geometry for responsive orbiting with
progressive implicit images from Fidget's software voxel renderer in a background
job. Standalone mesh and implicit projections remain available. With stock
disabled, it renders the blue reference cube instead. Drag to orbit
and scroll to zoom. The controls overlay the bottom of the full-pane 3D view;
the unlabelled slider seeks by cutting distance,
showing upcoming paths in gold and the tool in orange; completed paths disappear.
Edit/scrub the row counts, UV spacing,
mapping constants, colors, or explicit line radius. The latter controls visual
thickness, not cutter size. A separate Grap mapping offsets surface samples along
their normals by half the editable tool diameter (initially 0.125 inches).
The editable tilt defaults to 45°; each crossing family has a fixed cutter axis.
The example chooses the lean away from the vise using each operation's explicit
setup-up vector, pulls along that lean, and orders rows for clockwise climb
cutting. This is not yet a fixture collision check.
Path positions now locate the tip, after normal compensation and the axial
ball-center-to-tip shift. At the end of the toolpaths list, `ball tool` evaluates
the common constructor and `square tool` is an editable profile with a wider,
tapered non-cutting shank. Each displays a mirrored 2D profile beside its data.
The square tool is not yet used by the program: tool changes and chamfer passes
are the next step. See [tool profiles](../docs/tool-profiles.md).
No links between passes are implied. Mesh's optional `mesh depth` defaults to 6.
Full-stock implicit GPU rendering is unsupported,
so the software path is explicit, not a fallback after attempting a GPU render;
see [the limitation](../docs/toolpaths.md#playback).
The document includes its own copy of the cube definition;
its geometry drives the toolpath's contact points. Tan stock starts as a one-inch cube, bounded
by −0.5…0.5 on every axis (one model unit means one inch in this example); completed
cuts subtract continuous swept ball-end solids through Fidget. Both operations
together finish the six indents, but leave the chamfers untouched and do not
plan roughing, links, or collision clearance. Expand `playback` to edit the stock bounds and color; removing its
`stock` field returns to the wire envelope and reference model. Both preview
functions support playback and stock removal. Controls use per-view state
without making the document unsaved. The general dependency graph retains the
path recording and latest result. The combined preview shows its mesh while a
current implicit image is pending, then refines toward native resolution and
finally four times the depth samples for cleaner sharp edges. An
ellipsis remains until refinement completes. New camera or playback input
cancels the previous refinement sequence. The mesh
preview instead moves the tool immediately while its old stock mesh is desaturated.

See [toolpaths](../docs/toolpaths.md) for the streaming interface, its optional
recorder, and the boundary between this experiment and machining motion.
