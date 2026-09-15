# Release Checklist

## Fidget constant-field GPU validation

Verify the adopted upstream fix on a real GPU. The reviewed `0c89e87` Git pin
includes the fix; there is no local workaround. Also exercise multi-object
colors, resizing, and orbiting. The GPU-only regression test is explicit and
ignored by default because Seatbelt exposes no Metal adapter. See
[the tracked fix and report follow-up](deferred.md#fidget-constant-fields-on-the-gpu--verify-adopted-upstream-fix).

## Fidget implicit CAM rendering

Do not enable the full stock-removal scene on the GPU by default until the
2026-09-14 readback stall is understood. Async scheduling prevents UI blocking,
but cannot cancel already submitted GPU work. CPU captures and small GPU tests
pass; they do not validate this larger scene. The stock tape is now confirmed
to contain 794 loads/stores at 35% playback and 12,914 at completion; the pinned
GPU interpreter and simplifier leave the corresponding `OP_MEM` unimplemented.
Do not resubmit this unsupported program to the GPU merely to repeat the stall.
Command+9 explicitly uses software implicit rendering in the general background
executor. `editor_toolpath_implicit_async_svg_captures` verifies this path under
the build sandbox; native implicit GPU support needs an upstream fix.
See [toolpaths](toolpaths.md#playback).

## Fidget cold start

Resolve the first-use Metal pipeline stall before distributing a build with
Fidget support. With Progred's per-app Metal cache removed, the first 3D preview
took 5.23 seconds; 5.22 seconds of that was Fidget pipeline construction. The
same preview rendered in 4–6 ms after initialization.

That measurement used Fidget 0.5.0, which eagerly built roughly 44 compute
pipelines, including seven register-count variants for each of five pipeline
families. The `0c89e87` revision adopts upstream
[lazy pipeline construction](https://github.com/mkeeter/fidget/pull/492).
Repeat the cold-cache measurement, including the new color pass; do not assume
the stall is solved. Before release, move any remaining blocking initialization
behind a nonblocking placeholder. Precompiled Metal binary archives are a
longer-term alternative if the wgpu Metal backend gains useful support.
