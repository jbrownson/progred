use objc2_app_kit::{NSView, NSWindow};
use objc2_foundation::{NSPoint, NSString, NSURL};
use std::path::Path;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

/// AppKit's running cascade origin, threaded through window creation.
pub(crate) type CascadePoint = NSPoint;

pub(crate) fn initial_cascade() -> CascadePoint {
    NSPoint::new(0.0, 0.0)
}

fn with_appkit_window(window: &Window, f: impl FnOnce(&NSWindow)) {
    let RawWindowHandle::AppKit(handle) = window
        .window_handle()
        .expect("macOS window handle")
        .as_raw()
    else {
        unreachable!("macOS builds use AppKit window handles")
    };

    // SAFETY: Winit owns this main-thread NSView for at least as long
    // as `window`; the borrowed reference is used only for this call.
    let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    f(&view.window().expect("winit NSView window"));
}

/// The title-bar proxy icon: the document the window represents, with
/// AppKit's path popover and icon dragging.
pub(crate) fn set_represented(window: &Window, path: Option<&Path>) {
    let url =
        path.map(|path| NSURL::fileURLWithPath(&NSString::from_str(&path.display().to_string())));
    with_appkit_window(window, |appkit_window| {
        appkit_window.setRepresentedURL(url.as_deref());
    });
}

/// Adopts (or drops) the frame-autosave name of a live window — the
/// Save As path, where a nameless window gains its document identity.
pub(crate) fn set_autosave_name(window: &Window, name: Option<&str>) {
    with_appkit_window(window, |appkit_window| {
        let name = name.map(NSString::from_str).unwrap_or_default();
        if appkit_window.setFrameAutosaveName(&name) {
            if !name.is_empty() {
                // The frame as it stands at adoption; AppKit only
                // writes on its own for changes made after the name
                // is set.
                appkit_window.saveFrameUsingName(&name);
            }
        } else {
            // Rejected — another live window owns the name. AppKit
            // keeps the previous name in that case, which would go on
            // saving this window's frames under a document it no
            // longer shows; nameless is the honest state.
            appkit_window.setFrameAutosaveName(&NSString::new());
        }
    });
}

/// A live window's top-left in AppKit screen coordinates — the seed
/// for cascading a duplicate off its sibling.
pub(crate) fn top_left(window: &Window) -> CascadePoint {
    let mut point = NSPoint::new(0.0, 0.0);
    with_appkit_window(window, |appkit_window| {
        let frame = appkit_window.frame();
        point = NSPoint::new(frame.origin.x, frame.origin.y + frame.size.height);
    });
    point
}

/// Places one new window the way AppKit's document machinery does:
/// a named window's saved frame when one exists, else cascaded by
/// AppKit itself — from `seed` (a sibling's top-left) when given, else
/// the running cascade. A name persists future moves and resizes;
/// unclaimed windows stay nameless and always fresh.
pub(crate) fn place_and_autosave_frame(
    window: &Window,
    name: Option<&str>,
    seed: Option<CascadePoint>,
    cascade: &mut CascadePoint,
) {
    with_appkit_window(window, |appkit_window| {
        let name = name.map(NSString::from_str);
        let restored = name
            .as_deref()
            .is_some_and(|name| appkit_window.setFrameUsingName(name));
        if !restored {
            *cascade = appkit_window.cascadeTopLeftFromPoint(seed.unwrap_or(*cascade));
        }
        if let Some(name) = &name {
            // The claims map guarantees uniqueness; a refusal would be
            // a bookkeeping bug, answered by staying nameless.
            let _ = appkit_window.setFrameAutosaveName(name);
        }
    });
}
