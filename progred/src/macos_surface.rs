//! A Metal surface whose drawable presentation joins AppKit's Core Animation
//! transaction instead of trailing live window geometry.

use objc2_quartz_core::CAMetalLayer;
use vello::wgpu::{Instance, Surface, SurfaceTargetUnsafe};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

pub(crate) fn create(instance: &Instance, window: &Window) -> Surface<'static> {
    let RawWindowHandle::AppKit(handle) = window
        .window_handle()
        .expect("macOS window handle")
        .as_raw()
    else {
        unreachable!("macOS builds use AppKit window handles")
    };

    // SAFETY: Winit owns this NSView for at least as long as `window`.
    let layer = unsafe { raw_window_metal::Layer::from_ns_view(handle.ns_view) };
    // SAFETY: `Layer::as_ptr` is a valid CAMetalLayer for the lifetime of `layer`.
    unsafe { layer.as_ptr().cast::<CAMetalLayer>().as_ref() }.setPresentsWithTransaction(true);

    // SAFETY: Wgpu retains the CAMetalLayer while the surface is alive; the
    // window owning its parent view outlives the surface in `RenderState`.
    unsafe {
        instance.create_surface_unsafe(SurfaceTargetUnsafe::CoreAnimationLayer(
            layer.as_ptr().as_ptr(),
        ))
    }
    .expect("Error creating transactional Metal surface")
}
