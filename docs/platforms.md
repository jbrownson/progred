# Platforms

Progred has native macOS and Linux hosts and a browser host. They enter the
same Rust application rather than reimplementing the editor. macOS is the
primary development platform; the others don't have feature or performance
parity. How builds are isolated is in [build security](build-security.md).

## macOS

`make run` performs the sandboxed release build, packages and ad-hoc signs
`target/sandbox/app/Progred.app`, launches a fresh instance through Launch
Services with the App Sandbox entitlement, and waits for it to exit. Launch
Services doesn't attach the sandboxed GUI process to the terminal's standard
streams; use macOS logging for diagnostics. `make sandbox-app` builds the bundle
without launching it.

`make dev` keeps a development session open: Ctrl+C stops the current build or
app, rebuilds, and launches again; Ctrl+\ quits the session, as does closing the
terminal. Quitting the app also rebuilds and restarts it. A failed command waits
for Ctrl+C instead of retrying. Restarting discards unsaved changes. The loop,
`tools/dev-native`, owns a process group for each command; on macOS its small
native launcher opens the bundle through `NSWorkspace` and keeps the returned
`NSRunningApplication`, so termination targets that instance rather than every
process named `progred`. `python3 tools/test-dev-native.py` checks its process
and terminal behavior with disposable subprocesses.

## Linux

`make run`, `make dev`, `make images`, and the fetch, check, build, and test
targets go through `tools/linux-cargo`, which builds under bubblewrap into
`target/sandbox/build`; see [build security](build-security.md). Most desktop
distributions already have bubblewrap, since Flatpak uses it; otherwise install
the `bubblewrap` package. The app itself runs outside the sandbox. `make install-linux` builds a release binary and
installs it for the current user following the freedesktop conventions: the
binary in `~/.local/bin`, a desktop entry, and hicolor icons.

Winit loads its windowing libraries at runtime; under X11 that includes
`libxkbcommon-x11` (Debian's `libxkbcommon-x11-0`). Rendering needs a Vulkan or
OpenGL driver; Mesa's software Vulkan driver, lavapipe, works without a GPU.

## Browser

The browser build is supported on macOS and Linux, in the same build sandboxes
as the native app; `website/build-ci.sh` also builds it on a Linux CI runner for
publishing. It needs a secure context (localhost or HTTPS) and cross-origin
isolation for its shared-memory workers, so plain LAN HTTP or a `file://` URL
won't run it. Setup and the host's options are in
[the browser host notes](../web/README.md); the website is described in
[its README](../website/README.md).

## Other ports

An iPad feasibility host was removed; see [its notes](history/ipad-host.md).
visionOS and other ports are [deferred](deferred.md#additional-ports).
