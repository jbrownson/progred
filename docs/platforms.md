# Platform Work

Progred has native macOS and iPad shells plus a browser shell. The native
shells enter the same Rust application; platform projects package it rather
than reimplementing the editor.

## iPad

The browser build remains the shortest iPad loop: serve it from the development
Mac and open the Mac's LAN address in Safari. The native host builds Progred as
an `aarch64-apple-ios` static library; a minimal Xcode application calls its
exported entry point, after which Winit owns the UIKit lifecycle and WGPU/Vello
renders through Metal.

Install the Rust targets once:

```sh
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
```

Open `ios/Progred.xcodeproj`, select an Apple development team and a simulator
or paired iPad, then use Xcode's ordinary Run button. Its first build phase
builds the Rust static library for the selected SDK through the repository's
Seatbelt wrapper; Xcode then compiles the tiny Objective-C entry point, links,
signs, installs, and launches the application. Cargo still performs its normal
incremental check on every Xcode build, so Rust changes need no separate build
step.

`make build-ipad` and `make build-ipad-device` remain useful for unsigned CI or
command-line builds of the simulator and device forms. They use the same Xcode
build phase and therefore the same sandboxed Rust build.

The first host deliberately embeds the existing examples instead of adding
file management. Its clipboard is in-memory. Touch, Pencil-as-touch, and
indirect pointer events arrive through Winit; hardware-keyboard support awaits
testing, while the software keyboard still requires a UIKit text-input bridge.
Until a native discard prompt exists, switching documents in an edited iPad
session discards the in-memory document directly.

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
