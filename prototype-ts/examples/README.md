# Scene 3D Examples

These `.progred` files are loadable from the editor with `File -> Open`.

- `scene3d-implicit-sphere.progred` builds a sphere from primitive implicit expression nodes.
- `scene3d-two-blob-union.progred` uses `Minimum` to union two primitive sphere expressions.
- `scene3d-lens-intersection.progred` uses `Maximum` to intersect two primitive sphere expressions.

The files intentionally do not use a `Sphere` constructor. Each sphere-like shape is encoded as:

```text
(x - cx) * (x - cx) + (y - cy) * (y - cy) + (z - cz) * (z - cz) - r * r
```

Each `Implicit Solid` also carries meshing fields:

- `depth`: Fidget octree depth.
- `scale`: cubic half-size of the meshing domain.
