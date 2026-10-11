# Release checklist

Do these before distributing a build. The upstream research behind them is in
[history](history/dependency-reviews-2026.md).

## Fidget constant-field GPU validation

Fidget 0.5.0 bound a zero-byte variable buffer for constant fields, which WebGPU
validation rejects on every platform. Fidget 0.5.1 includes upstream's fix
([#461](https://github.com/mkeeter/fidget/pull/461)); there is no local
workaround. Verify it on a real GPU: run the ignored
`gpu_colors_constants_and_workspace_reuse` test, which covers zero, positive,
negative, and computed constant fields and multi-object colors without CPU
fallback, and exercise resizing and orbiting by hand. If validation still fails,
prepare a minimal standalone Fidget reproducer with the exact validation error,
dependency versions, OS, and adapter for an upstream report; #461 covers the
original defect, so don't file a duplicate of it. Don't forbid constant fields
in the Fidget language or merely hide their completions.

## Fidget implicit CAM rendering

Keep the full stock-removal scene off the GPU by default until the 2026-09-14
readback stall is understood. Async scheduling prevents UI blocking but can't
cancel already submitted GPU work. The stock tape spills (794 loads and stores
at 35% playback, 12,914 at completion), and Fidget's GPU interpreter leaves
`OP_MEM` unimplemented, so Command+3 renders implicit images in software; see
[toolpaths](toolpaths.md#playback). Don't resubmit this program to the GPU
merely to repeat the stall. Ask upstream whether the GPU interpreter should
implement or reject `OP_MEM`; no issue has been filed yet.

## Fidget cold start

Resolve the first-use Metal pipeline stall. With Progred's per-app Metal cache
removed, Fidget 0.5.0 spent 5.22 of the first 3D preview's 5.23 seconds building
roughly 44 compute pipelines; later previews rendered in 4–6 ms. Fidget 0.5.1
builds pipelines lazily ([#492](https://github.com/mkeeter/fidget/pull/492)).
Repeat the cold-cache measurement, including the color pass; don't assume the
stall is solved. Move any remaining blocking initialization behind a
nonblocking placeholder. Precompiled Metal binary archives are a longer-term
alternative if wgpu's Metal backend gains useful support.
