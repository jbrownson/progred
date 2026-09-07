# Examples

Open these files normally to edit and save them. The Examples menu opens a fresh
copy instead; its shortcuts are Command+1…8 on macOS and Ctrl+1…8 in the drawn menu.

| Shortcut | Document | Purpose |
| --- | --- | --- |
| 1 | `sample.gid` | Basic data and projections |
| 2 | `grap-demo.gid` | Grap evaluation |
| 3 | `iop-tree.gid` | Editable tree drawing, inspired by Inventing on Principle |
| 4 | `fidget.gid` | Small constructive-geometry example |
| 5 | `fidget-torus.gid` | Smooth torus: a small arithmetic field |
| 6 | `fidget-tanglecube.gid` | Polynomial surface with several handles |
| 7 | `fidget-gyroid.gid` | Dense trigonometric lattice clipped to a sphere |
| 8 | `fidget-cube.gid` | Rhino-derived fidget cube: concave quadratic faces and planar chamfers |

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

The left viewport calls the function and passes its result to `preview 3d`.
Its explicit bounds are −0.6…0.6, in the same model units; no geometry scaling
or cube-specific Rust primitive is involved. Orbit and zoom work normally.
See [the geometry derivation](../docs/fidget-cube.md) for correspondence to the
Rhino surfaces, parameter limitations, and what is not yet a CAM model.

See [frame performance checks](../docs/performance.md) for repeatable orbit
measurements using the actual example documents.
