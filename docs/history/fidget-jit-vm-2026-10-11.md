# Fidget's JIT against its VM on Apple Silicon, 2026-10-11

`make perf` on an M3 Pro on AC power, the JIT build then one with Fidget's VM
in its place, through a temporary `software-vm` feature:

| Workload | JIT | VM |
| --- | --- | --- |
| `stock_render` | 6.29 s | 1.51 s |
| `stock_meshing/1.00` | 438 ms | 442 ms |
| `fidget_orbit` | 16.1 ms | 16.1 ms |

Only the stock render runs in software, and the VM renders it four times
faster: the JIT's code generation outweighs its evaluation on a program that
size. The GPU workloads don't change. The example's figures went the other way,
the JIT about twice as fast on the ring, torus and tanglecube, but those
render in a millisecond or two either way, while the gyroid took 46 ms on the
JIT and 36 ms on the VM. The VM then became the software evaluator everywhere, and
the JIT was removed. Three default runs that day, on
battery, on AC power after two release builds, and on AC power with the machine
idle, spread by up to 18% on the heavy workloads, the idle one slowest, so a
difference under that on a stock or Fidget workload is drift, not a change.

`cam_mesh_roundtrip_profile` asked for the direct and image-roundtrip
composites to be identical, and they differ in about 0.08% of pixels, mostly by
a level or two, with larger differences along the mesh's silhouette, where the
roundtrip's intermediate image resolves edge coverage differently. The check
passed once and failed twice that day, with either evaluator; it now allows
0.1% of pixels to differ by more than two levels. The VM run was recorded with
`--only "stock|fidget_"` before that.

The drift rule and the round-trip tolerance remain in
[performance checks](../performance.md).
