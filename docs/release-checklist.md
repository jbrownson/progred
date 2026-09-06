# Release Checklist

## Fidget constant-field GPU validation

Adopt and verify the upstream fix on a real GPU. The pinned 0.5.0 can fail when
rendering constant fields; there is deliberately no local workaround. See
[the tracked fix and report follow-up](deferred.md#fidget-constant-fields-on-the-gpu--upstream-fix-to-adopt).

## Fidget cold start

Resolve the first-use Metal pipeline stall before distributing a build with
Fidget support. With Progred's per-app Metal cache removed, the first 3D preview
took 5.23 seconds; 5.22 seconds of that was Fidget pipeline construction. The
same preview rendered in 4–6 ms after initialization.

Fidget 0.5.0 eagerly builds roughly 44 compute pipelines, including seven
register-count variants for each of five pipeline families. Before release,
initialize this work asynchronously behind a nonblocking placeholder and
investigate lazy specialization in Fidget. Precompiled Metal binary archives
are a longer-term alternative if the wgpu Metal backend gains useful support.
