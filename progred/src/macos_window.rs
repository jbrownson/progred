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

/// Claims the first free autosave name for the window's document —
/// doc:{path}, then doc:{path}:2, :3, … — so the standard continuous
/// autosave machinery applies with its uniqueness rule satisfied by
/// numbering rather than administered. Which window holds which
/// number, and thus which saved geometry a reopen inherits, is
/// deliberately unspecified.
fn claim_name(appkit_window: &NSWindow, path: &Path) -> objc2::rc::Retained<NSString> {
    let mut n = 1usize;
    loop {
        let candidate = if n == 1 {
            format!("doc:{}", path.display())
        } else {
            format!("doc:{}:{n}", path.display())
        };
        let candidate = NSString::from_str(&candidate);
        if appkit_window.setFrameAutosaveName(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Places one new window the way AppKit's document machinery does:
/// the claimed name's saved frame when one exists, else cascaded from
/// the previous window by AppKit itself; continuous autosave persists
/// the frame from then on. Untitled windows stay nameless and always
/// fresh.
pub(crate) fn place_and_autosave_frame(
    window: &Window,
    path: Option<&Path>,
    cascade: &mut CascadePoint,
) {
    with_appkit_window(window, |appkit_window| {
        let restored = path
            .map(|path| claim_name(appkit_window, path))
            .is_some_and(|name| appkit_window.setFrameUsingName(&name));
        if !restored {
            *cascade = appkit_window.cascadeTopLeftFromPoint(*cascade);
        }
    });
}

/// Save As: the window's document changed, so its autosave identity
/// follows — a fresh numbered claim for the new path. Assigning a
/// name reloads that name's saved frame, which would yank the window
/// to wherever that document's window last sat; the frame is put
/// back and snapshotted as the name's new one instead.
pub(crate) fn rename_document_frame(window: &Window, path: &Path) {
    with_appkit_window(window, |appkit_window| {
        let frame = appkit_window.frame();
        let name = claim_name(appkit_window, path);
        if appkit_window.frame() != frame {
            appkit_window.setFrame_display(frame, false);
        }
        appkit_window.saveFrameUsingName(&name);
    });
}
