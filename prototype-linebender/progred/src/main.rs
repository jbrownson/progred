//! Window shell: winit + Vello plumbing around pure frame drawing.
//! `draw_frame` writes to any puri `Canvas`; here it streams into vello.

use std::sync::Arc;

use parley::{FontContext, GenericFamily, LayoutContext, StyleProperty};
use puri::draw::{Canvas, GlyphRun, Shape};
use puri::edit::{EditCtx, EditStyle, LineEditState, text_edit};
use puri::handler::{Handler, HasHandler, ImeEvent};
use puri::layout::{Extent, HAlign, Node, col, decorate, leaf, pad, place_top_left, row};
use puri::text::{TextCtx, TextStyle, paragraph, text};
use puri_vello::VelloCanvas;
use ui_events::keyboard::{Key, NamedKey};
use ui_events::pointer::{PointerButtonEvent, PointerEvent};
use ui_events_winit::{WindowEventReducer, WindowEventTranslation};
use vello::kurbo::{Affine, Insets, Point, Rect, RoundedRect, Stroke, Vec2};
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
                self.model
                    .field
                    .refresh(&mut self.font_cx, &mut self.layout_cx, scale as f32);
                let mut frame = Frame {
                    scene: None,
                    ime_caret: None,
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

                self.model
                    .field
                    .refresh(&mut self.font_cx, &mut self.layout_cx, scale as f32);
                self.scene.reset();
                let mut frame = Frame {
                    scene: Some(&mut self.scene),
                    ime_caret: None,
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
                let ime_caret = frame.ime_caret;
                drop(frame);

                window.set_ime_allowed(self.model.editing);
                if let Some(caret) = ime_caret {
                    window.set_ime_cursor_area(
                        winit::dpi::PhysicalPosition::new(caret.x0, caret.y0),
                        winit::dpi::PhysicalSize::new(caret.width(), caret.height()),
                    );
                }

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
    let mut field = LineEditState::new("click to edit me", 15.0);
    field
        .editor
        .edit_styles()
        .insert(StyleProperty::Brush(Color::WHITE.into()));
    field.editor.edit_styles().insert(GenericFamily::SystemUi.into());

    let mut app = App {
        context: RenderContext::new(),
        renderers: vec![],
        state: RenderState::Suspended(None),
        scene: Scene::new(),
        font_cx: FontContext::new(),
        layout_cx: LayoutContext::new(),
        model: Model {
            selected: None,
            field,
            editing: false,
        },
        reducer: WindowEventReducer::default(),
    };

    let event_loop = EventLoop::new().expect("Couldn't create event loop");
    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");
}

struct Model {
    selected: Option<usize>,
    field: LineEditState,
    editing: bool,
}

const SWATCHES: usize = 8;

fn move_selection(model: &mut Model, delta: isize) {
    model.selected = Some(match model.selected {
        None => {
            if delta >= 0 {
                0
            } else {
                SWATCHES - 1
            }
        }
        Some(i) => (i as isize + delta).rem_euclid(SWATCHES as isize) as usize,
    });
}

/// One pass over the UI: read-only in the model, producing draw calls
/// (when a scene is attached) and a transient `Handler`. Every event
/// runs the pass fresh, dispatches through the returned handler — all
/// mutation happens there, after placement — and discards it. The
/// dispatch context is `App`, so dispatches reach the model and the
/// measurement caches parley's driver needs.
struct Frame<'a> {
    scene: Option<&'a mut Scene>,
    ime_caret: Option<Rect>,
    handler: Handler<App>,
}

impl HasHandler<App> for Frame<'_> {
    fn handler(&mut self) -> &mut Handler<App> {
        &mut self.handler
    }
}

fn edit_access(app: &mut App) -> EditCtx<'_> {
    EditCtx {
        state: &mut app.model.field,
        fonts: &mut app.font_cx,
        layouts: &mut app.layout_cx,
    }
}

fn pointer_position(event: &PointerButtonEvent) -> Point {
    Point::new(event.state.position.x, event.state.position.y)
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
    let panel_rect = panel.rect();
    frame.handler.on_pointer_down(move |app: &mut App, event| {
        panel_rect.contains(pointer_position(event)) && {
            app.model.selected = None;
            app.model.editing = false;
            true
        }
    });

    for i in 0..SWATCHES {
        let t = i as f32 / (SWATCHES - 1) as f32;
        let x = m + padding + i as f64 * 34.0 * scale;
        let y = height - m - padding - 24.0 * scale;
        let swatch = Rect::new(x, y, x + 24.0 * scale, y + 24.0 * scale);
        frame.fill(
            swatch,
            Color::new([0.25 + 0.65 * t, 0.42, 0.9 - 0.55 * t, 1.0]),
            Affine::IDENTITY,
        );
        if model.selected == Some(i) {
            frame.stroke(
                swatch.inflate(3.0 * scale, 3.0 * scale),
                Stroke::new(2.0 * scale),
                Color::WHITE,
                Affine::IDENTITY,
            );
        }
        frame.handler.on_pointer_down(move |app: &mut App, event| {
            swatch.contains(pointer_position(event)) && {
                app.model.selected = Some(i);
                true
            }
        });
    }

    frame.handler.on_key(|app: &mut App, event| {
        event.state.is_down()
            && match &event.key {
                Key::Named(NamedKey::ArrowLeft) => {
                    move_selection(&mut app.model, -1);
                    true
                }
                Key::Named(NamedKey::ArrowRight) => {
                    move_selection(&mut app.model, 1);
                    true
                }
                Key::Named(NamedKey::Escape) => {
                    if app.model.editing {
                        app.model.editing = false;
                    } else {
                        app.model.selected = None;
                    }
                    true
                }
                _ => false,
            }
    });

    let mut tcx = TextCtx {
        fonts: font_cx,
        layouts: layout_cx,
        scale: scale as f32,
    };
    let title_style = TextStyle {
        size: 28.0,
        brush: Color::WHITE.into(),
        weight: Some(650.0),
    };
    let body_style = TextStyle {
        size: 15.0,
        brush: Color::WHITE.into(),
        weight: None,
    };

    let title = text(&mut tcx, "Progred on Puri", &title_style);
    let body = paragraph(
        &mut tcx,
        "The loop is closed: the same pure pass runs for every event and every \
         redraw — an input pass carries the event and whoever takes it first owns \
         it. Click a swatch to select it, arrow keys to move the selection, Escape \
         or the panel background to clear it.",
        &body_style,
        1.4,
        (width - 2.0 * m - 2.0 * padding) as f32,
    );

    let num = text(&mut tcx, "a + b", &body_style);
    let den = text(&mut tcx, "2", &body_style);
    let bar_width = num.extent.width.max(den.extent.width) + 8.0 * scale;
    let fraction = col(
        HAlign::Center,
        1,
        3.0 * scale,
        vec![
            num,
            rule(bar_width, 1.5 * scale, Color::new([0.85, 0.85, 0.9, 1.0]).into()),
            den,
        ],
    );
    let equation = row(
        4.0 * scale,
        vec![
            text(&mut tcx, "area =", &body_style),
            fraction,
            text(&mut tcx, "mm²", &body_style),
        ],
    );

    // A traditional text box, composed from the bare editable text:
    // pad for insets, decorate for chrome and pointer wiring.
    let edit_style = EditStyle {
        selection: Color::new([0.25, 0.45, 0.85, 0.45]).into(),
        cursor: Color::WHITE.into(),
    };
    let inset = 8.0 * scale;
    let editing = model.editing;
    let caret_local = editing.then(|| model.field.cursor_rect()).flatten();
    let field = text_edit(&model.field, editing, &edit_style, edit_access);
    let field_box = decorate(
        pad(Insets::uniform(inset), field),
        move |p: &mut Frame, rect| {
            let box_shape = RoundedRect::from_rect(rect, 6.0 * scale);
            p.fill(box_shape, Color::new([0.16, 0.17, 0.20, 1.0]), Affine::IDENTITY);
            if editing {
                p.stroke(
                    box_shape,
                    Stroke::new(1.5 * scale),
                    Color::new([0.45, 0.6, 0.95, 1.0]),
                    Affine::IDENTITY,
                );
            }
            let origin = Vec2::new(rect.x0 + inset, rect.y0 + inset);
            if let Some(caret) = caret_local {
                p.ime_caret = Some(caret + origin);
            }
            p.handler.on_pointer_down(move |app: &mut App, event| {
                let position = pointer_position(event);
                rect.contains(position) && {
                    app.model.editing = true;
                    let EditCtx {
                        state,
                        fonts,
                        layouts,
                    } = edit_access(app);
                    state.pointer_down(
                        fonts,
                        layouts,
                        position - origin,
                        event.state.modifiers.shift(),
                        event.state.count,
                    );
                    true
                }
            });
        },
    );

    let content = col(
        HAlign::Start,
        0,
        14.0 * scale,
        vec![title, body, equation, field_box],
    );
    place_top_left(content, frame, Point::new(m + padding, m + padding));
}

fn rule<P: Canvas>(width: f64, thickness: f64, brush: Brush) -> Node<P> {
    leaf(
        Extent {
            width,
            ascent: thickness,
            descent: 0.0,
        },
        move |canvas: &mut P, at| {
            canvas.fill(
                Rect::new(at.x, at.y - thickness, at.x + width, at.y),
                brush,
                Affine::IDENTITY,
            );
        },
    )
}
