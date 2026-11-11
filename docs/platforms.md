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

### Scrolling

How far a scroll moves on Linux is a convention, not something the platform
says, and Progred follows Zed's (November 2026, measured on Hyprland with
Wayland).

Wayland reports a touchpad swipe as a distance in surface units, the units
pointer motion uses, so read literally a swipe scrolls as far as the fingers
moved. Nothing reads it literally. A wheel's notch arrives through the same
event as a small number, 10 to 15 units, which no application could scroll as
that many pixels, so each toolkit chose what a notch is worth and scales by
it, and swipes inherited the scaling. Compositors in turn scale swipes down
before applications see them (Hyprland's touchpad `scroll_factor`, 0.4 in
Omarchy's configuration), which only suits applications that scale them back
up. The two factors roughly cancel; an application with only one of them is
slow. Progred scrolled swipes one for one and a notch 40 points, and so moved
a third as far as Zed for the same swipe.

macOS has none of this: the system applies acceleration and momentum and hands
the application the distance to scroll. Windows gives a notch a fixed unit and
a system setting for how many lines it is. Linux has no setting applications
agree on.

So on Linux a swipe's reported distance is multiplied by 3 and a notch scrolls
three lines of text, in
[`scroll.rs`](../progred/src/display/widget/scroll.rs) (`SWIPE_SCALE`,
`WHEEL_LINE`). Both are Zed's numbers, from its Wayland client
(`let modifier = 3.0` on continuous axis values, `SCROLL_LINES = 3.0` for a
notch), which gives no reason for either. Winit reports a notch as a line and
a swipe as pixels on Wayland; under X11 it reports both as lines, which the
swipe factor doesn't reach, and that hasn't been tried.

There is no momentum after the fingers lift. libinput leaves it to
applications, which get a stop event to start it from, and GTK, Firefox and
Chromium each implement their own; Zed and VS Code don't, and Progred
doesn't. If the two numbers suit one setup and not another, the next step is
a sensitivity setting, as both of those editors have, not a better constant.

## Browser

The browser build is supported on macOS and Linux, in the same build sandboxes
as the native app; `website/build-ci.sh` also builds it on a Linux CI runner for
publishing. Its shared-memory workers need cross-origin isolation, which
needs a secure context (localhost or HTTPS); elsewhere, as on plain LAN HTTP,
the page loads a fallback build without workers, whose 3D toolpath previews show
a note instead. A `file://` URL runs neither. Setup and the host's options are in
[the browser host notes](../web/README.md); the website is described in
[its README](../website/README.md).

## Other ports

An iPad feasibility host was removed; see [its notes](history/ipad-host.md).
visionOS and other ports are [deferred](deferred.md#additional-ports).
