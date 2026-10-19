//! Readback adapter for headless GPU assertions and round-trip comparisons only.
use super::*;

use puri_vello::capture;

pub(crate) struct Renderer {
    gpu: capture::Gpu,
    renderer: puri_vello::mesh::Renderer,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new(pollster::block_on(fidget_engine::wgpu::Gpu::init_basic()).unwrap())
    }
}

pub(crate) fn raster(
    geometry: &Mesh,
    preview: &VolumePreview,
    state: Option<&Value>,
    scale: f64,
    renderer: &mut Renderer,
) -> Option<ImageData> {
    raster_surface(geometry, None, preview, state, scale, renderer)
}

pub(crate) fn raster_surface(
    geometry: &Mesh,
    surface: Option<Surface>,
    preview: &VolumePreview,
    state: Option<&Value>,
    scale: f64,
    renderer: &mut Renderer,
) -> Option<ImageData> {
    let pixels = raster_size(preview.size, scale)?;
    let view = view(preview, camera(state), pixels)?;
    Some(rgba_image(
        pixels.width(),
        pixels.height(),
        renderer.render(geometry, &view, surface)?,
    ))
}

impl Renderer {
    pub(super) fn new(gpu: fidget_engine::wgpu::Gpu) -> Self {
        let gpu = capture::Gpu {
            device: gpu.device,
            queue: gpu.queue,
        };
        let renderer = puri_vello::mesh::Renderer::new(&gpu.device, &gpu.queue);
        Self { gpu, renderer }
    }

    pub(super) fn render(
        &mut self,
        geometry: &Mesh,
        view: &View,
        surface: Option<Surface>,
    ) -> Option<Vec<u8>> {
        let texture = self.renderer.render(geometry, view, surface.as_ref())?;
        let premultiplied = ImageData {
            data: self.gpu.read(&texture).into(),
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::AlphaPremultiplied,
            width: texture.width(),
            height: texture.height(),
        };
        Some(puri::draw::straight_rgba(&premultiplied)?.into_owned())
    }
}
