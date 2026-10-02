# Examples

Open these files normally to edit and save them. The Examples menu opens a fresh
copy instead. Use Command plus the number below on macOS, or Ctrl on other
platforms. Examples are numbered consecutively in menu order.

| Shortcut | Document | Purpose |
| --- | --- | --- |
| 1 | `iop-tree.gid` | Editable tree drawing, inspired by Inventing on Principle |
| 2 | `fidget-shapes.gid` | Cutaway spheres, torus, tanglecube, gyroid, and the plain fidget cube, drawn by figure libraries the document declares |
| 3 | `toolpaths.gid` | Two-operation CAM playback, progressive stock rendering, and tool profiles |
| 4 | `grap-demo.gid` | Grap evaluation |
| 5 | `libraries.gid` | Libraries the document defines: fractions, angles, and tints, each a key it owns and a view that draws it |
| | `navigation.gid` | Keyboard navigation: nested outlines, custom code forms, lists/cells, missing values, shared occurrences, and computed results |

The menu lists the examples meant for visitors. The navigation fixture isn't on
it; open the file directly.

## Keyboard navigation

Open `navigation.gid` with **File → Open**.

Arrow navigation follows the chosen layout's logical lines. Left/Right walk
reading order, including selectable containers, and wrap to the next/previous
line. Up/Down select the first stop on the adjacent logical line. Multiline
containers add a leading entry line, shared by consecutive enclosing containers;
single-line containers precede their contents on that same line. Text handles
its own arrows before yielding at an edge. There is no geometric scoring or
directional history.

Also try these layout corner cases:

- **Deep multiline containers:** Right visits each nested cell; Down skips the
  enclosing chain to the first content line. Up returns to the outermost cell.
- **Side-by-side columns:** a document-authored layout interleaves the columns'
  logical rows. The values remain ordinary editable document data.
- **Offset baselines:** the left column's last child aligns with the right
  column's first. From “Aligned with lower left,” Left reaches “Lower left,”
  Up reaches “Middle left,” and Down reaches “Below that.” Navigation follows
  declared baseline children, not painted heights or pixel distances.
- **Label after a block:** a vertical list with “Beside the first item” to its
  right. Select the whole list (its bracket) and keep pressing Right: list →
  first item → “Beside the first item” → second item. Content after a block
  stays on the line it is drawn on; only content before a block joins its
  entry line. Narrow the window if the list sits inline.
- **Single-line value after a block:** the same list with a one-item list to
  its right. From the first item, Right reaches the small list, then its item,
  then the second item of the stacked list.

See [layout navigation](../docs/navigation.md) for the current contract.

## Fidget shapes

**Examples → Implicit CAD Shapes** (Command+2 / Ctrl+2) lists five figures, records
like `{figure: torus}`, and declares the two libraries that draw them: `figure`
previews with `preview 3d`, `mesh figure` with `preview mesh`. Each draws its
figure's picture beside the shape's editable source. Every shape is a no-argument
Grap function, since a library's view receives the record as data and calls the
function to get the shape. There is no separate pane or chooser; scroll to another
shape, or fold its source using the ordinary editor controls. The full machining
example remains separate at Command+3 / Ctrl+3.

The original `fidget.gid`, `fidget-torus.gid`, `fidget-tanglecube.gid`,
`fidget-gyroid.gid`, and `fidget-cube.gid` files remain as stable fixtures for
rendering regressions and historical performance comparisons. They and
`sample.gid` can still be opened as files, but are no longer separate menu items.

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

The cube entry in `fidget-shapes.gid` contains a no-argument Grap function which
constructs ordinary Fidget arithmetic. Its `where` bindings expose the Rhino defaults: size `1`,
chamfer `0.1`, and control-point depth `0.5`. The resulting face-center depression
is `0.125`, not `0.5`. Edit or scrub those constants in the source.

Its `mesh figure` view calls the function, and `meshed` wraps the result in a cyan
scene object for `preview mesh` at mesh depth 5. The cube function itself still returns an ordinary field.
Its explicit bounds are −0.6…0.6, in the same model units; no geometry scaling
or cube-specific Rust primitive is involved. Orbit and zoom work normally, with
fresh CPU meshing on every frame and GPU triangle drawing on native platforms.
Edit `meshed` to change `mesh depth`, or to call `preview 3d` for comparison.
See [the mesh viewport](../docs/fidget-mesh.md) for parameters and limitations.
See [the geometry derivation](../docs/fidget-cube.md) for correspondence to the
Rhino surfaces, parameter limitations, and what is not yet a CAM model.

Colors are editable RGB values in the documents. The cutaway-spheres entry uses
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
plan roughing, links, or collision clearance. The Model / Stock control above the
timeline switches between the blue target model with a stock wireframe and the
tan remaining material, retaining camera, ranges, and playback position. Model
is the default; stock bounds and color remain editable in the preview function. Both preview
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
