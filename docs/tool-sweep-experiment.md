# Analytic cutter-sweep experiment

2026-09-15. This is an isolated investigation, **not a production geometry
contract or a completed replacement for profile approximation**. The code is
test-only in `progred/src/libraries/toolpath/cutter/tests/analytic.rs`.

## Decision

Do not promote the current closed-form candidate. Its sampled inside/outside
classification agrees with an independent numerical reference, but real Fidget
meshing produces non-finite vertices. Point evaluation is not a sufficient test:
meshing also depends on interval evaluation and gradients.

Keep the existing exact line-segment and ball-end sweeps, and the explicit
chord approximation for other curved profiles. A subsequent cleanup separated
simulation accuracy from tool geometry: `profile tolerance` now belongs to
playback settings, and the profile picture chooses its own display accuracy.
The experimental analytic kernel remains test-only; neither the app's sweep
algorithm nor Fidget itself has changed.

## What the literature supplies

[Chung et al., Modeling the surface swept by a generalized cutter for NC
verification](https://pure.seoultech.ac.kr/en/publications/modeling-the-surface-swept-by-a-generalized-cutter-for-nc-verific/)
describes toroidal cutter sweeps requiring quartic root finding. Its result is
a single-valued swept surface, not a drop-in volumetric Fidget expression.
We did not obtain the full paper; this comparison uses its published abstract.

[Roth et al., Surface Swept by a Toroidal Cutter during 5-Axis
Machining](https://cs.uwaterloo.ca/~smann/Papers/CAD.00.02.pdf) distinguishes the
fixed silhouette of straight, fixed-orientation translation from the changing
imprint curves of simultaneous five-axis motion. It develops a numerical
construction for the latter. This supports numerical treatment as a practical
choice, not a claim that every five-axis move lacks an analytic solution.

## Candidate construction

The experiment derives a solid-membership test, rather than copying the
papers' surface construction. It handles one convex bull mill oriented along Z,
with a quarter-circle corner of radius `r`, outer tool radius `R + r`, and a
finite cylindrical flute. It does not solve arbitrary profile arcs or changing
orientation. It is an implicit field, **not a signed distance function**.

In the rounded axial band, let `rho` be distance from the axis and `h` distance
from the corner circle's axial center. The filled radial disk is inside if any
of these conditions holds:

```
rho² - R² <= 0
A = rho² + h² + R² - r² <= 0
F = A² - 4 R² rho² <= 0
```

The first two conditions avoid the hollow/spindle-torus ambiguity of using
the quartic alone. Restrict motion to the interval where that axial band is
present, then minimize each inequality on that interval. The first two are
quadratic minimizations. For `F`, endpoints plus the real roots of its cubic
derivative suffice.

Centering distance along the normalized movement ray at its closest point to
the torus center removes the cubic term and avoids division by the fourth
power of move length. With ray direction `v`, closest-point vector `q`, and
`A0 = |q|² + R² - r²`, the quartic is:

```
F(u) = u⁴
     + (2 A0 - 4 R² (vx² + vy²)) u²
     + 8 R² (qx vx + qy vy) u
     + A0² - 4 R² (qx² + qy²)
```

Cardano's real and trigonometric branches are emitted as ordinary Fidget
operations. Finite band intervals, the upper cylinder, and the whole tool's
end caps are handled explicitly. Internal band joins are not capped.

This is still a numerical implementation of an analytic formula. Small-value
guards in the experimental cube-root evaluation are not a validated error
bound. In particular, algebraic correctness does not guarantee stable f32
gradients at repeated roots or branch boundaries. Selecting constant limiting
angles instead of differentiating `acos(clamp(x))` avoids one singularity,
but does **not** eliminate the observed meshing failure. Its exact remaining
cause has not been isolated; no upstream Fidget bug is asserted here.

## Verification

Run the opt-in experiments through the repository build sandbox:

```
./tools/sandbox-cargo test --release -p progred bull_sweep --lib -- --ignored --nocapture --test-threads=1
```

The point test compares actual Fidget f32 evaluation with an independent f64
convex minimization along the move. It includes stationary, axial, transverse,
oblique, reversed-direction, and very short moves; small through nearly
hemispherical corner radii; a three-dimensional grid; and points on either
side of numerically located boundaries. The numerical reference is specific to
this convex cutter, not a claimed solver for every non-convex tool.

The meshing test exercises actual interval/gradient-driven octree construction
at depths 5 and 6. Its final assertion is intentionally a **promotion gate**:
the experimental candidate currently fails it. The existing chord-based
implementation is the comparison, not a fallback hidden inside the candidate.
Both tests are ignored by ordinary test runs.

### Local results

The serial release run evaluated 10,232 points for each of 28 cutter/move
combinations. The analytic candidate had no non-finite point values and no
sign disagreements where the reference field's magnitude exceeded `1e-5`.
The chord comparison passed its looser `0.002` margin, appropriate to the
requested `0.001` profile approximation. These are sampled checks, not a proof
of a geometric error bound.

For a radius-0.5 bull mill with corner radius 0.1, flute length 1.2, and the
move `(0,0,0) -> (1,0,0.5)`:

| Implementation | Mesh depth | Time | Non-finite vertices |
| --- | --- | --- | --- |
| Analytic candidate | 5 | 41.6 ms | 56 / 6,168 |
| Profile chords | 5 | 17.3 ms | 0 / 6,218 |
| Analytic candidate | 6 | 187.5 ms | 249 / 13,361 |
| Profile chords | 6 | 59.6 ms | 0 / 12,234 |

Those are single-run diagnostic measurements, not benchmark medians. Point
evaluation alone was comparable or faster for larger rounded corners, but
meshing was slower and invalid. There is no demonstrated end-to-end gain to
justify shipping this formula or expanding it into a general arc solver now.

## Next useful work

- Approximation settings and geometric validation are now separate from tool
  sampling. Static circular profiles need not be approximated in an implicit
  interpreter; that is still a possible improvement. Tessellated display does
  need an accuracy policy.
- Keep exact continuous fixed-axis sweeps of straight profile bands. Chords
  approximate curved profile geometry, not the translation between path points.
- Before promoting any analytic arc kernel, test gradients, interval pruning,
  scale, tangencies, interior joins, and complete stock scenes—not just signs
  or a single mesh. Do not silently substitute clamped normals or bad vertices.
- For simultaneous five-axis motion, first specify pose interpolation and a
  geometric error budget. Adapt subdivision to translation and orientation;
  angular error must account for the distance of tool material from the
  rotation origin. Do not call fixed-orientation pieces exact rotating sweeps.
- Collision verification and machine/controller interpolation remain separate
  from this idealized simulation. No machining-ready clearance claim follows
  from these tests.
