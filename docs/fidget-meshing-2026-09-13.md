# Fidget cube meshing experiment — 2026-09-13

Subsequent work added an [in-app mesh viewport](fidget-mesh.md), explicitly
without caching. This report describes the earlier generation experiment.

This is a headless experiment, not a replacement viewport. It uses the actual
`examples/fidget-cube.gid` document: evaluate its viewport function, take the
returned implicit field and bounds, then mesh in model coordinates. Camera
rotation is applied only when drawing the resulting mesh, not during meshing.
The experiment uses Fidget's existing Manifold Dual Contouring implementation
at the already pinned revision `0c89e87e1b3a6d15cc0976ab6ff05a09f9cf91d6`.
No upstream update or JIT is involved.

## Reproduce

```sh
./tools/sandbox-cargo test --release -p progred --lib \
  --features mesh-experiment fidget_cube_mesh_experiment -- --ignored --nocapture
```

This opt-in feature enables `fidget/mesh`; the experiment itself is test-only.
It writes `timings.csv`, four STLs, solid/wire PNG pairs, a comparison SVG, and
a CPU voxel reference PNG into `target/sandbox/build/mesh-experiment`.
Rasterize the whole comparison without Quick Look's cropping:

```sh
rsvg-convert target/sandbox/build/mesh-experiment/cube-comparison.svg \
  -o target/sandbox/build/mesh-experiment/cube-comparison.png
```

## Measurements

Apple M3 Pro, 11 physical cores, optimized release profile without shipping LTO,
CPU VM evaluator and Fidget's default global thread pool. Two process runs;
each depth has one separately reported first build followed by three builds
whose median is below. These are short exploratory measurements, not thresholds.

| Octree depth | Finest grid spacing in model units | Triangles | Run 1 median | Run 2 median | Packed geometry |
| --- | ---: | ---: | ---: | ---: | ---: |
| 5 | 0.0375 | 18,430 | 10.4 ms | 9.7 ms | 324 KiB |
| 6 | 0.01875 | 40,570 | 28.2 ms | 26.4 ms | 713 KiB |
| 7 | 0.009375 | 59,850 | 86.7 ms | 86.5 ms | 1,052 KiB |
| 8 | 0.0046875 | 150,138 | 397.6 ms | 333.5 ms | 2,639 KiB |

The domain is `[-0.6, 0.6]³`. The grid is adaptive; depth is not a uniform
triangle density or a geometric error tolerance. Timings include octree
construction and triangle extraction, but exclude validation, image creation,
and file output. Viewport evaluation/library setup takes 1.7–2.3 ms and VM
lowering another 0.08–0.10 ms, measured separately. Packed geometry assumes f32
positions and u32 triangle indices, excluding normals and GPU resources; this
is not the resident size of Fidget's Rust mesh, which uses usize indices.

## Quality and limits

All four meshes have finite coordinates and field samples. Every indexed edge
has two incident triangles with consistent winding. These checks do **not**
establish a valid solid: they do not test vertex neighborhoods, self-intersection,
or missed features. Upstream explicitly warns about self-intersections and
features smaller than the sampling grid.

The mesher emits 182 / 578 / 786 / 311 exactly zero-area triangles at depths
5 / 6 / 7 / 8; respectively 34 / 128 / 24 / 0 contain coincident vertices.
The experiment reports and retains these, rather than silently welding or
repairing them. This needs investigation before using exported meshes as
manufacturing geometry. No such validity claim is made here.

The test samples the original field at vertices and triangle centroids. At
depth 6 the largest absolute residuals are about 0.000381 and 0.000208.
The cube field is not an exact signed distance, so these are **not distance
error bounds**, and they do not decrease monotonically at every depth.

Visual checks preserve the cube's curved depressions and chamfers at all four
settings; coarse facets are visible at depth 5. The preview uses a small
test-only CPU triangle rasterizer with flat, two-sided lighting, not the
application renderer. Its lighting differs from the voxel reference. Wire
overlays at high density alias heavily at the diagnostic image resolution.

## Direction suggested by the experiment

Depth 6 is a reasonable starting point for a viewport trial. Keep the implicit
field as the model, retain a derived mesh until the field/bounds/meshing settings
change, and apply camera changes only while drawing ordinary triangles. Render
toolpaths as explicit line/tube geometry alongside it rather than meshing a
union of more than a thousand implicit capsules. This experiment meshes the
cube only, not the combined toolpath field.

A GPU triangle renderer, its integration with the existing canvas, retained
mesh ownership, and asynchronous rebuild/cancellation are **not implemented**
by this experiment. There is no measured orbit frame rate yet. The result
establishes affordable mesh generation for this model, not general meshing
performance or CAD/CAM accuracy.
