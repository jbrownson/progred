//! Window shell: winit + Vello plumbing around pure frame drawing.
//! `run_frame` writes to any puri `Canvas`; here it streams into vello.

mod conventions;
mod identicon;
mod raw;

use std::sync::Arc;

use parley::{FontContext, LayoutContext};
use progred_graph::Id;
use puri::draw::{Canvas, GlyphRun, Shape};
use puri::handler::{Handler, HasHandler, ImeEvent};
use puri::layout::{HAlign, col, place_top_left};
use puri::text::{TextCtx, TextStyle, text};
use puri_vello::VelloCanvas;
use ui_events::pointer::PointerEvent;
use ui_events_winit::{WindowEventReducer, WindowEventTranslation};
use vello::kurbo::{Affine, Point, RoundedRect, Stroke};
use vello::peniko::{Brush, Color};
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::{self, CurrentSurfaceTexture};
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{Ime, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

enum RenderState {
    Active {
        surface: Box<RenderSurface<'static>>,
        valid_surface: bool,
        window: Arc<Window>,
    },
    Suspended(Option<Arc<Window>>),
}

struct App {
    context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    state: RenderState,
    scene: Scene,
    font_cx: FontContext,
    layout_cx: LayoutContext<Brush>,
    model: Model,
    reducer: WindowEventReducer,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let RenderState::Suspended(cached_window) = &mut self.state else {
            return;
        };

        let window = cached_window.take().unwrap_or_else(|| {
            let attr = Window::default_attributes()
                .with_inner_size(LogicalSize::new(900, 640))
                .with_title("Progred");
            Arc::new(event_loop.create_window(attr).unwrap())
        });

        let size = window.inner_size();
        let surface_future = self.context.create_surface(
            window.clone(),
            size.width,
            size.height,
            wgpu::PresentMode::AutoVsync,
        );
        let surface = pollster::block_on(surface_future).expect("Error creating surface");

        self.renderers
            .resize_with(self.context.devices.len(), || None);
        self.renderers[surface.dev_id].get_or_insert_with(|| {
            Renderer::new(
                &self.context.devices[surface.dev_id].device,
                RendererOptions::default(),
            )
            .expect("Couldn't create renderer")
        });

        self.state = RenderState::Active {
            surface: Box::new(surface),
            valid_surface: true,
            window,
        };
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let RenderState::Active { window, .. } = &self.state {
            self.state = RenderState::Suspended(Some(window.clone()));
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let (window, width, height) = match &self.state {
            RenderState::Active {
                surface, window, ..
            } if window.id() == window_id => (
                window.clone(),
                surface.config.width,
                surface.config.height,
            ),
            _ => return,
        };
        let scale = window.scale_factor();

        if !matches!(
            event,
            WindowEvent::KeyboardInput {
                is_synthetic: true,
                ..
            }
        ) {
            let ime = match &event {
                WindowEvent::Ime(ime) => Some(match ime {
                    Ime::Enabled => ImeEvent::Enabled,
                    Ime::Disabled => ImeEvent::Disabled,
                    Ime::Preedit(text, cursor) => ImeEvent::Preedit(text.clone(), *cursor),
                    Ime::Commit(text) => ImeEvent::Commit(text.clone()),
                }),
                _ => None,
            };
            let translation = self.reducer.reduce(scale, &event);
            if ime.is_some() || translation.is_some() {
                let mut frame = Frame {
                    scene: None,
                    handler: Handler::new(),
                };
                run_frame(
                    &mut frame,
                    &self.model,
                    &mut self.font_cx,
                    &mut self.layout_cx,
                    width as f64,
                    height as f64,
                    scale,
                );
                let handler = frame.handler;
                let handled = match (ime, translation) {
                    (Some(ime), _) => handler.dispatch_ime(self, &ime),
                    (None, Some(WindowEventTranslation::Keyboard(key_event))) => {
                        handler.dispatch_key(self, &key_event)
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Down(button)))) => {
                        handler.dispatch_pointer_down(self, &button)
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Move(update)))) => {
                        handler.dispatch_pointer_move(self, &update)
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Up(button)))) => {
                        handler.dispatch_pointer_up(self, &button)
                    }
                    _ => false,
                };
                if handled {
                    window.request_redraw();
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                let RenderState::Active {
                    surface,
                    valid_surface,
                    ..
                } = &mut self.state
                else {
                    return;
                };
                if size.width != 0 && size.height != 0 {
                    self.context
                        .resize_surface(surface, size.width, size.height);
                    *valid_surface = true;
                } else {
                    *valid_surface = false;
                }
            }

            WindowEvent::RedrawRequested => {
                if matches!(
                    &self.state,
                    RenderState::Active {
                        valid_surface: false,
                        ..
                    }
                ) {
                    return;
                }

                self.scene.reset();
                let mut frame = Frame {
                    scene: Some(&mut self.scene),
                    handler: Handler::new(),
                };
                run_frame(
                    &mut frame,
                    &self.model,
                    &mut self.font_cx,
                    &mut self.layout_cx,
                    width as f64,
                    height as f64,
                    scale,
                );
                drop(frame);

                let RenderState::Active { surface, .. } = &mut self.state else {
                    return;
                };
                let device_handle = &self.context.devices[surface.dev_id];

                self.renderers[surface.dev_id]
                    .as_mut()
                    .unwrap()
                    .render_to_texture(
                        &device_handle.device,
                        &device_handle.queue,
                        &self.scene,
                        &surface.target_view,
                        &vello::RenderParams {
                            base_color: Color::new([0.06, 0.065, 0.08, 1.0]),
                            width,
                            height,
                            antialiasing_method: AaConfig::Msaa16,
                        },
                    )
                    .expect("failed to render to texture");

                let surface_texture = match surface.surface.get_current_texture() {
                    CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
                    CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Suboptimal(_) => {
                        self.context.configure_surface(surface);
                        window.request_redraw();
                        return;
                    }
                    CurrentSurfaceTexture::Occluded | CurrentSurfaceTexture::Timeout => {
                        window.request_redraw();
                        return;
                    }
                    CurrentSurfaceTexture::Lost => panic!("Surface was lost"),
                    CurrentSurfaceTexture::Validation => {
                        panic!("Validation error getting surface")
                    }
                };

                let mut encoder =
                    device_handle
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Surface Blit"),
                        });
                surface.blitter.copy(
                    &device_handle.device,
                    &mut encoder,
                    &surface.target_view,
                    &surface_texture
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default()),
                );
                device_handle.queue.submit([encoder.finish()]);
                surface_texture.present();

                device_handle.device.poll(wgpu::PollType::Poll).unwrap();
            }
            _ => {}
        }
    }
}

fn main() {
    let mut app = App {
        context: RenderContext::new(),
        renderers: vec![],
        state: RenderState::Suspended(None),
        scene: Scene::new(),
        font_cx: FontContext::new(),
        layout_cx: LayoutContext::new(),
        model: Model {
            doc: raw::sample_document(),
            selection: None,
            collapse: raw::Collapse::default(),
        },
        reducer: WindowEventReducer::default(),
    };

    let event_loop = EventLoop::new().expect("Couldn't create event loop");
    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");
}

struct Model {
    doc: raw::Document,
    selection: Option<Id>,
    collapse: raw::Collapse,
}

/// One pass over the UI: read-only in the model, producing draw calls
/// (when a scene is attached) and a transient `Handler`. Every event
/// runs the pass fresh, dispatches through the returned handler — all
/// mutation happens there, after placement — and discards it. The
/// dispatch context is `App`, so dispatches reach the model and the
/// measurement caches parley's driver needs.
struct Frame<'a> {
    scene: Option<&'a mut Scene>,
    handler: Handler<App>,
}

impl HasHandler<App> for Frame<'_> {
    fn handler(&mut self) -> &mut Handler<App> {
        &mut self.handler
    }
}

impl Canvas for Frame<'_> {
    fn fill(&mut self, shape: impl Into<Shape>, brush: impl Into<Brush>, transform: Affine) {
        if let Some(scene) = self.scene.as_deref_mut() {
            VelloCanvas(scene).fill(shape, brush, transform);
        }
    }

    fn stroke(
        &mut self,
        shape: impl Into<Shape>,
        style: Stroke,
        brush: impl Into<Brush>,
        transform: Affine,
    ) {
        if let Some(scene) = self.scene.as_deref_mut() {
            VelloCanvas(scene).stroke(shape, style, brush, transform);
        }
    }

    fn glyph_run(&mut self, run: GlyphRun) {
        if let Some(scene) = self.scene.as_deref_mut() {
            VelloCanvas(scene).glyph_run(run);
        }
    }

    fn clip(&mut self, shape: impl Into<Shape>, transform: Affine, content: impl FnOnce(&mut Self)) {
        let shape = shape.into();
        if let Some(scene) = self.scene.as_deref_mut() {
            VelloCanvas(scene).push_clip(&shape, transform);
        }
        content(self);
        if let Some(scene) = self.scene.as_deref_mut() {
            VelloCanvas(scene).pop_clip();
        }
    }
}

fn run_frame(
    frame: &mut Frame<'_>,
    model: &Model,
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<Brush>,
    width: f64,
    height: f64,
    scale: f64,
) {
    let m = 24.0 * scale;
    let padding = 20.0 * scale;

    let panel = RoundedRect::new(m, m, width - m, height - m, 12.0 * scale);
    frame.fill(panel, Color::new([0.11, 0.12, 0.145, 1.0]), Affine::IDENTITY);
    frame.stroke(
        panel,
        Stroke::new(1.5 * scale),
        Color::new([0.32, 0.35, 0.42, 1.0]),
        Affine::IDENTITY,
    );
    let mut tcx = TextCtx {
        fonts: font_cx,
        layouts: layout_cx,
        scale: scale as f32,
    };
    let title_style = TextStyle {
        size: 22.0,
        brush: Color::WHITE.into(),
        weight: Some(650.0),
    };
    let styles = raw::RawStyles::new(scale);

    let title = text(&mut tcx, "Progred", &title_style);
    let body = raw::project(
        &model.doc,
        model.selection.as_ref(),
        &model.collapse,
        &mut tcx,
        &styles,
        |app: &mut App, id| app.model.selection = Some(id),
    );
    let content = col(HAlign::Start, 0, 18.0 * scale, vec![title, body]);
    place_top_left(content, frame, Point::new(m + padding, m + padding));
}
