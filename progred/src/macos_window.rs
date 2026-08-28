use objc2_app_kit::NSView;
use objc2_foundation::NSString;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

pub(crate) fn autosave_frame(window: &Window, name: &str) {
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
    let appkit_window = view.window().expect("winit NSView window");
    assert!(
        appkit_window.setFrameAutosaveName(&NSString::from_str(name)),
        "unique AppKit window frame autosave name"
    );
}
