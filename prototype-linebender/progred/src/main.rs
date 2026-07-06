//! Window shell: winit + Vello plumbing around pure frame drawing.
//! `run_frame` writes to any puri `Canvas`; here it streams into vello.

mod conventions;
mod identicon;
mod raw;
// Unwired: give it a view toggle when identicons next need tuning.
#[allow(dead_code)]
mod sheet;

use std::sync::Arc;

use parley::{FontContext, LayoutContext};
use puri::draw::{Canvas, GlyphRun, Shape};
use puri::edit::EditCtx;
use puri::handler::{Handler, HasHandler, ImeEvent};
use puri::layout::place_top_left;
use puri::text::TextCtx;
use puri_vello::VelloCanvas;
use ui_events::pointer::{PointerButton, PointerEvent};
use ui_events_winit::{WindowEventReducer, WindowEventTranslation};
use vello::kurbo::{Affine, Point, Stroke};
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
        let window = match &self.state {
            RenderState::Active { window, .. } if window.id() == window_id => window.clone(),
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
                self.refresh_edit(scale as f32);
                let mut frame = Frame {
                    scene: None,
                    handler: Handler::new(),
                    descends: Vec::new(),
                };
                run_frame(
                    &mut frame,
                    &self.model,
                    &mut self.font_cx,
                    &mut self.layout_cx,
                    scale,
                );
                let Frame {
                    handler, descends, ..
                } = frame;
                let handled = match (ime, translation) {
                    (Some(ime), _) => handler.dispatch_ime(self, &ime),
                    // Keys nothing claims fall through to selection
                    // stepping, so the selected string's editor always
                    // wins over navigation.
                    (None, Some(WindowEventTranslation::Keyboard(key_event))) => {
                        handler.dispatch_key(self, &key_event)
                            || match raw::step_selection(
                                &descends,
                                self.model.selection.as_ref(),
                                &key_event,
                            ) {
                                Some(path) => {
                                    self.model.selection =
                                        Some(raw::Selection::edge(&self.model.doc, path));
                                    true
                                }
                                None => false,
                            }
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
                    let model = &mut self.model;
                    if let Some(selection) = &model.selection {
                        raw::sync_edit(&mut model.doc, selection);
                    }
                    window.request_redraw();
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            // KNOWN ISSUE: a live drag-resize glitches on macOS (the
            // compositor stretches a frame mid-drag). Not ours — vello's
            // own examples show it, and this is the canonical handling
            // (resize + request_redraw). The real fix is below wgpu
            // (CAMetalLayer `presentsWithTransaction` / a synchronized
            // drawable commit); accepted for now, revisit in a lower layer.
            WindowEvent::Resized(size) => {
                if let RenderState::Active {
                    surface,
                    valid_surface,
                    ..
                } = &mut self.state
                {
                    if size.width != 0 && size.height != 0 {
                        self.context
                            .resize_surface(surface, size.width, size.height);
                        *valid_surface = true;
                        window.request_redraw();
                    } else {
                        *valid_surface = false;
                    }
                }
            }

            WindowEvent::RedrawRequested => self.redraw(),
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
    selection: Option<raw::Selection>,
    collapse: raw::Collapse,
}

/// One pass over the UI: read-only in the model, producing draw calls
/// (when a scene is attached), a transient `Handler`, and the list of
/// `Descend`s placed this frame. Every event runs the pass fresh; the
/// handler and descends drive dispatch and selection, then are
/// discarded. The dispatch context is `App`, so dispatches reach the
/// model and the measurement caches parley's driver needs.
struct Frame<'a> {
    scene: Option<&'a mut Scene>,
    handler: Handler<App>,
    descends: Vec<raw::Descend>,
}

impl HasHandler<App> for Frame<'_> {
    fn handler(&mut self) -> &mut Handler<App> {
        &mut self.handler
    }
}

impl raw::HasDescends for Frame<'_> {
    fn descends(&mut self) -> &mut Vec<raw::Descend> {
        &mut self.descends
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

impl App {
    /// The selection's line-editor layout is lazy and refreshing needs
    /// `&mut`; run it before each pure pass.
    fn refresh_edit(&mut self, scale: f32) {
        if let Some(line) = self.model.selection.as_mut().and_then(raw::Selection::edit_mut) {
            line.refresh(&mut self.font_cx, &mut self.layout_cx, scale);
        }
    }

    /// Renders the current model to the surface, from `RedrawRequested`.
    fn redraw(&mut self) {
        let RenderState::Active {
            surface,
            valid_surface: true,
            window,
        } = &self.state
        else {
            return;
        };
        let window = window.clone();
        let scale = window.scale_factor();
        let width = surface.config.width;
        let height = surface.config.height;

        self.refresh_edit(scale as f32);
        self.scene.reset();
        let mut frame = Frame {
            scene: Some(&mut self.scene),
            handler: Handler::new(),
            descends: Vec::new(),
        };
        run_frame(
            &mut frame,
            &self.model,
            &mut self.font_cx,
            &mut self.layout_cx,
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
                    base_color: Color::new([0.965, 0.965, 0.972, 1.0]),
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
}

fn run_frame(
    frame: &mut Frame<'_>,
    model: &Model,
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<Brush>,
    scale: f64,
) {
    // Empty space deselects. Registered before the content places, so
    // the descend handlers (registered as they place) take precedence,
    // and only a press that claims no edge falls through to here.
    frame.handler().on_pointer_down(|app: &mut App, event| {
        event.button == Some(PointerButton::Primary) && app.model.selection.take().is_some()
    });

    let mut tcx = TextCtx {
        fonts: font_cx,
        layouts: layout_cx,
        scale: scale as f32,
    };
    let styles = raw::RawStyles::new(scale);
    let body = raw::project(
        &model.doc,
        model.selection.as_ref(),
        &model.collapse,
        &mut tcx,
        &styles,
        // Re-clicking the selected path keeps its editor state.
        |app: &mut App, path| {
            if !app.model.selection.as_ref().is_some_and(|s| s.path() == path) {
                app.model.selection = Some(raw::Selection::edge(&app.model.doc, path));
            }
        },
        edit_ctx,
    );
    place_top_left(body, frame, Point::new(28.0 * scale, 28.0 * scale));
}

/// Dispatch-time access to the selection's editor. Only handlers the
/// mounted editor registered call this, and the pass mounts it only
/// when the selection carries one, so it exists at dispatch.
fn edit_ctx(app: &mut App) -> EditCtx<'_> {
    let state = app
        .model
        .selection
        .as_mut()
        .and_then(raw::Selection::edit_mut)
        .expect("edit dispatch without a string selection");
    EditCtx {
        state,
        fonts: &mut app.font_cx,
        layouts: &mut app.layout_cx,
    }
}
