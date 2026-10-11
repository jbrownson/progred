# Release Checklist

## Fidget constant-field GPU validation

Verify the adopted upstream fix on a real GPU. The reviewed `0c89e87` Git pin
includes the fix; there is no local workaround. Also exercise multi-object
colors, resizing, and orbiting. The GPU-only regression test is explicit and
ignored by default because Seatbelt exposes no Metal adapter.

Fidget 0.5.0's GPU `RenderShape` allocates its variable buffer from the compiled
variable count. A constant field therefore creates a zero-byte buffer and tries
to bind it as storage. Creating a zero-sized WebGPU buffer is allowed; binding
a zero-sized range is not. This is platform-independent validation, not a Metal
quirk; see [WebGPU buffer creation](https://gpuweb.github.io/gpuweb/#buffer-creation)
and [bind-group validation](https://gpuweb.github.io/gpuweb/#dom-gpudevice-createbindgroup).

Upstream checked on 2026-09-05 at main commit
[`94123ea`](https://github.com/mkeeter/fidget/blob/94123ea4a351fedf66df404c31f3fed0e1474b0e/fidget-wgpu/src/lib.rs#L280-L293):
this is already fixed by allocating at least four bytes. The change landed on
2026-08-25 in [PR #461](https://github.com/mkeeter/fidget/pull/461), commit
[`19a9049`](https://github.com/mkeeter/fidget/commit/19a904926a72a4e8509d02060bfef109fe37e488).
Searches of open/closed issues and PRs for constants, buffers, and bindings found
no separate matching bug report. The older [PR #318](https://github.com/mkeeter/fidget/pull/318)
fixes a different constant-evaluation problem. Do not file a duplicate report
for the allocation/binding defect already fixed in #461.

The 2026-09-13 dependency update adopts this fix through the reviewed upstream
revision `0c89e87e1b3a6d15cc0976ab6ff05a09f9cf91d6`. There is no constant-field
workaround. The ordinary CPU backend remains available when no GPU is available
and on the web; its tests do not verify GPU behavior.

Still test zero, positive, negative, and computed constant fields on the real
GPU. The ignored `gpu_colors_constants_and_workspace_reuse` test exercises
these and multi-object colors without allowing CPU fallback, but its attempted
run inside Seatbelt found no Metal adapter. Normal CPU tests pass.
If validation still fails, prepare a minimal standalone Fidget reproducer and
capture the exact validation error, dependency versions, OS, and adapter/backend
for an upstream report. The original Progred crash did not preserve the GPU
validation message, so its precise cause remains inferred from this code path.
Metal itself is unavailable inside the build sandbox. Do not forbid constant
fields in the Fidget language or merely hide their completions.

## Fidget implicit CAM rendering

Do not enable the full stock-removal scene on the GPU by default until the
2026-09-14 readback stall is understood. Async scheduling prevents UI blocking,
but cannot cancel already submitted GPU work. CPU captures and small GPU tests
pass; they do not validate this larger scene. The stock tape is now confirmed
to contain 794 loads/stores at 35% playback and 12,914 at completion; the pinned
GPU interpreter and simplifier leave the corresponding `OP_MEM` unimplemented.
Do not resubmit this unsupported program to the GPU merely to repeat the stall.
Command+3 explicitly uses software implicit rendering in the general background
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
