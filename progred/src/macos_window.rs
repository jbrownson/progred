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

/// Places one new window the way AppKit's document machinery does:
/// a named window's saved frame when one exists, else cascaded from
/// the previous window by AppKit itself. A name persists future moves
/// and resizes; untitled windows stay nameless and always fresh.
pub(crate) fn place_and_autosave_frame(
    window: &Window,
    name: Option<&str>,
    cascade: &mut CascadePoint,
) {
    with_appkit_window(window, |appkit_window| {
        let name = name.map(NSString::from_str);
        let restored = name
            .as_deref()
            .is_some_and(|name| appkit_window.setFrameUsingName(name));
        if !restored {
            *cascade = appkit_window.cascadeTopLeftFromPoint(*cascade);
        }
        if let Some(name) = &name {
            assert!(
                appkit_window.setFrameAutosaveName(name),
                "unique AppKit window frame autosave name"
            );
        }
    });
}
