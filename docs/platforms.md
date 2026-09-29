# Platform Work

Progred has native macOS and Linux shells plus a browser shell. They enter the
same Rust application rather than reimplementing the editor. An iPad
feasibility host was removed; see [its notes](history/ipad-host.md).

## Browser

The browser build requires HTTPS and cross-origin isolation for shared-memory
workers; plain LAN HTTP is no longer sufficient. See [the browser host](../web/README.md).

## Apple Vision Pro

A visionOS version is interesting specifically for CAD/CAM: the same GID/Grap
model could project into spatial geometry while ordinary windows retain source,
parameters, and other editor projections. This should be treated as a spatial
projection/backend design, not merely the two-dimensional editor placed in a
headset.

Questions for that spike include the RealityKit/Metal boundary, spatial
selection and manipulation, and how existing source-to-output provenance maps
onto three-dimensional geometry. None requires changing the GID or Grap model
in advance.
