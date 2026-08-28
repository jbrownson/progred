# Deferred Platform Work

Progred currently has a native macOS shell and a browser shell. Additional
ports are deliberately deferred until focused product work has advanced; they
are useful bounded projects for lower-energy development time.

## iPad

The browser build is the shortest iPad loop: serve it from the development Mac
and open the Mac's LAN address in Safari. A native version is also plausible:
winit supplies the UIKit event loop and touch input, while wgpu supplies Metal.
The initial native target should embed built-in examples rather than solve file
management.

A native port would need:

- a small Xcode host calling Progred built as an `aarch64-apple-ios` static
  library;
- desktop-only dependency gates for clipboard, dialogs, menus, and launch
  behavior;
- a UIKit text-input bridge, because winit currently supplies touch but not
  iOS keyboard events; and
- a `make run-ipad` path that signs, installs, and launches on a paired iPad.

After initial pairing and enabling Developer Mode, Xcode can deploy development
builds to an iPad over the local network. Rebuilding and refreshing the browser
version should remain the faster hot-tub iteration path.

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
