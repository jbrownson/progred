//! Browser presentation through the native compositor, with Canvas2D fallback
//! when WebGPU is unavailable. GPU setup happens before entering Winit.
use puri::draw::CanvasSink;
use puri_vello::compositor::{Compositor, Resources, SplitCanvas};
use std::cell::RefCell;
use vello::{
    peniko::Color,
    util::{RenderContext, RenderSurface},
    wgpu::{self, CurrentSurfaceTexture},
};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::HtmlCanvasElement;

thread_local! {
    static PREPARED: RefCell<Option<Renderer>> = const { RefCell::new(None) };
}

pub enum Renderer {
    Gpu(Box<Gpu>),
    Canvas(puri_web::WebCanvas),
}

pub struct Gpu {
    context: RenderContext,
    surface: RenderSurface<'static>,
    compositor: Compositor,
    resources: Resources,
}

#[wasm_bindgen]
pub async fn prepare_renderer(canvas: HtmlCanvasElement) -> Result<String, JsValue> {
    let renderer = Renderer::new(canvas).await?;
    let name = renderer.name().to_owned();
    PREPARED.with(|slot| *slot.borrow_mut() = Some(renderer));
    Ok(name)
}

pub(crate) fn take() -> Renderer {
    PREPARED.with(|slot| {
        slot.borrow_mut()
            .take()
            .expect("prepare_renderer before start_editor")
    })
}

impl Renderer {
    pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let mut context = RenderContext::new();
        // Ask for the adapter before binding the canvas to a context type;
        // browsers don't allow switching an existing canvas from GPU to 2D.
        if context.device(None).await.is_none() {
            let canvas = canvas
                .get_context("2d")?
                .ok_or("Canvas2D unavailable")?
                .dyn_into::<web_sys::CanvasRenderingContext2d>()?;
            return Ok(Self::Canvas(puri_web::WebCanvas(canvas)));
        }
        let surface = context
            .create_surface(
                wgpu::SurfaceTarget::Canvas(canvas.clone()),
                canvas.width().max(1),
                canvas.height().max(1),
                wgpu::PresentMode::AutoVsync,
            )
            .await
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let device = &context.devices[surface.dev_id];
        let compositor = Compositor::new(&device.device, &device.queue)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        Ok(Self::Gpu(Box::new(Gpu {
            context,
            surface,
            compositor,
            resources: Resources::default(),
        })))
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Gpu(_) => "WebGPU",
            Self::Canvas(_) => "Canvas2D",
        }
    }

    /// True if presented; false asks the shell to retry an unavailable surface.
    pub fn render(
        &mut self,
        width: u32,
        height: u32,
        background: Color,
        draw: impl FnOnce(&mut dyn CanvasSink),
    ) -> Result<bool, String> {
        match self {
            Self::Canvas(canvas) => {
                canvas.clear(width.into(), height.into(), background);
                draw(canvas);
                Ok(true)
            }
            Self::Gpu(gpu) => {
                let Gpu {
                    context,
                    surface,
                    compositor,
                    resources,
                } = &mut **gpu;
                if surface.config.width != width || surface.config.height != height {
                    context.resize_surface(surface, width, height);
                }
                let device = &context.devices[surface.dev_id];
                let mut paint = SplitCanvas::default();
                draw(&mut paint);
                let output = compositor
                    .render(
                        &device.device,
                        &device.queue,
                        &paint.finish(),
                        resources,
                        &surface.target_texture,
                        background,
                    )
                    .map_err(|e| e.to_string())?;
                let texture = match surface.surface.get_current_texture() {
                    CurrentSurfaceTexture::Success(texture) => texture,
                    CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Suboptimal(_) => {
                        context.configure_surface(surface);
                        return Ok(false);
                    }
                    CurrentSurfaceTexture::Occluded | CurrentSurfaceTexture::Timeout => {
                        return Ok(false);
                    }
                    CurrentSurfaceTexture::Lost => {
                        return Err("WebGPU surface lost; reload to retry".into());
                    }
                    CurrentSurfaceTexture::Validation => {
                        return Err("WebGPU surface validation failed".into());
                    }
                };
                let mut encoder = device.device.create_command_encoder(&Default::default());
                surface.blitter.copy(
                    &device.device,
                    &mut encoder,
                    &output.texture.create_view(&Default::default()),
                    &texture.texture.create_view(&Default::default()),
                );
                device.queue.submit([encoder.finish()]);
                texture.present();
                Ok(true)
            }
        }
    }
}
