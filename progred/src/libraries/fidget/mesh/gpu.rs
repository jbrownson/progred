//! Readback adapter for headless GPU assertions and round-trip comparisons only.
use super::*;

#[path = "../../../../tests/compositor_experiment/gpu.rs"]
#[allow(dead_code)]
mod capture;

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
    Some(ImageData {
        data: renderer.render(geometry, &view, surface)?.into(),
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::Alpha,
        width: pixels.width(),
        height: pixels.height(),
    })
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
        let mut rgba = self.gpu.read(&texture);
        // Match the historical ImageData boundary for byte/pixel comparisons.
        for pixel in rgba.chunks_exact_mut(4) {
            let alpha = u32::from(pixel[3]);
            if alpha > 0 && alpha < 255 {
                for channel in &mut pixel[..3] {
                    *channel = ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
                }
            }
        }
        Some(rgba)
    }
}
