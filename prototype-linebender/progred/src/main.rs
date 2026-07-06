//! Window shell: winit + Vello plumbing around pure frame drawing.
//! `run_frame` writes to any puri `Canvas`; here it streams into vello.

mod conventions;
mod identicon;
mod raw;
// Unwired: give it a view toggle when identicons next need tuning.
#[allow(dead_code)]
mod sheet;
mod store;

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use parley::{FontContext, LayoutContext};
use puri::draw::{Canvas, GlyphRun, Shape};
use puri::edit::EditCtx;
use puri::handler::{Handler, HasHandler, ImeEvent};
use puri::layout::place_top_left;
use puri::text::TextCtx;
use puri_vello::VelloCanvas;
use ui_events::keyboard::{Key, KeyboardEvent};
use ui_events::pointer::{PointerButton, PointerEvent};
use ui_events_winit::{WindowEventReducer, WindowEventTranslation};
use vello::kurbo::{Affine, Point, Stroke};
use vello::peniko::{Brush, Color};
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::{self, CurrentSurfaceTexture};
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
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
    /// Where the document lives; `None` is untitled until the first
    /// save asks for a path.
    doc_path: Option<PathBuf>,
    /// Attached to the app once launched; commands arrive as user
    /// events.
    menu: Menu,
    menu_ids: MenuIds,
    reducer: WindowEventReducer,
}

struct MenuIds {
    open: MenuId,
    save: MenuId,
    save_as: MenuId,
}

/// The menu bar: file commands own their key equivalents, so the
/// platform routes Cmd+S and friends here rather than through key
/// dispatch. Attachment is macOS-only until another platform is run.
fn build_menu() -> (Menu, MenuIds) {
    let accel = if cfg!(target_os = "macos") {
        Modifiers::META
    } else {
        Modifiers::CONTROL
    };
    let open = MenuItem::new("Open…", true, Some(Accelerator::new(Some(accel), Code::KeyO)));
    let save = MenuItem::new("Save", true, Some(Accelerator::new(Some(accel), Code::KeyS)));
    let save_as = MenuItem::new(
        "Save As…",
        true,
        Some(Accelerator::new(Some(accel | Modifiers::SHIFT), Code::KeyS)),
    );
    let menu = Menu::new();
    let ids = MenuIds {
        open: open.id().clone(),
        save: save.id().clone(),
        save_as: save_as.id().clone(),
    };
    menu.append_items(&[
        &Submenu::with_items(
            "Progred",
            true,
            &[
                &PredefinedMenuItem::about(None, None),
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::quit(None),
            ],
        )
        .expect("app menu"),
        &Submenu::with_items(
            "File",
            true,
            &[
                &open,
                &PredefinedMenuItem::separator(),
                &save,
                &save_as,
            ],
        )
        .expect("file menu"),
    ])
    .expect("menu bar");
    (menu, ids)
}

fn dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("progred", &["progred"])
}

impl ApplicationHandler<MenuEvent> for App {
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: MenuEvent) {
        if *event.id() == self.menu_ids.open {
            self.menu_open();
        } else if *event.id() == self.menu_ids.save {
            self.menu_save(false);
        } else if *event.id() == self.menu_ids.save_as {
            self.menu_save(true);
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // After launch, so winit cannot replace it (its own default
        // menu is disabled at loop construction).
        #[cfg(target_os = "macos")]
        self.menu.init_for_nsapp();

        let RenderState::Suspended(cached_window) = &mut self.state else {
            return;
        };

        let window = cached_window.take().unwrap_or_else(|| {
            let attr = Window::default_attributes()
                .with_inner_size(LogicalSize::new(900, 640))
                .with_title(self.title());
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
                let viewport = window.inner_size().height as f64;
                let mut frame = Frame {
                    scene: None,
                    handler: Handler::new(),
                    descends: Vec::new(),
                    max_scroll: 0.0,
                };
                run_frame(
                    &mut frame,
                    &self.model,
                    &mut self.font_cx,
                    &mut self.layout_cx,
                    scale,
                    viewport,
                );
                let Frame {
                    handler,
                    descends,
                    max_scroll,
                    ..
                } = frame;
                let handled = match (ime, translation) {
                    (Some(ime), _) => handler.dispatch_ime(self, &ime),
                    // Keys nothing claims fall through to the collapse
                    // toggle and then selection stepping, so the
                    // selected string's editor always wins over both.
                    (None, Some(WindowEventTranslation::Keyboard(key_event))) => {
                        handler.dispatch_key(self, &key_event)
                            || self.collapse_key(&key_event)
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
                    // Scrolls nothing claims move the document.
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Scroll(update)))) => {
                        handler.dispatch_scroll(self, &update)
                            || self.scroll_document(&update, scale, viewport, max_scroll)
                    }
                    _ => false,
                };
                if handled {
                    let model = &mut self.model;
                    if let Some(selection) = &model.selection {
                        raw::write_through(&mut model.doc, selection);
                    }
                    window.request_redraw();
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            // KNOWN ISSUE: a live drag-resize can still glitch on macOS
            // (the compositor stretches a stale frame mid-drag). Not
            // ours — vello's own examples show it. Rendering the new
            // size synchronously inside the resize event narrows the
            // stale window; the real fix is below wgpu (CAMetalLayer
            // `presentsWithTransaction` / a synchronized drawable
            // commit). Revisit in a lower layer.
            WindowEvent::Resized(size) => {
                let valid = size.width != 0 && size.height != 0;
                if let RenderState::Active {
                    surface,
                    valid_surface,
                    ..
                } = &mut self.state
                {
                    if valid {
                        self.context
                            .resize_surface(surface, size.width, size.height);
                    }
                    *valid_surface = valid;
                }
                if valid {
                    self.redraw();
                }
            }

            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }
}

fn main() {
    let doc_path = std::env::args().nth(1).map(PathBuf::from);
    // A given-but-missing path is a new document there; no path is
    // untitled until the first save asks. A file that exists but does
    // not parse is refused rather than silently replaced, so a save
    // cannot clobber it with the sample.
    let doc = match &doc_path {
        Some(path) if path.exists() => store::load(path).unwrap_or_else(|error| {
            eprintln!("failed to load {}: {error}", path.display());
            std::process::exit(1);
        }),
        _ => raw::sample_document(),
    };

    let mut builder = EventLoop::<MenuEvent>::with_user_event();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        builder.with_default_menu(false);
    }
    let event_loop = builder.build().expect("Couldn't create event loop");
    // The menu attaches to the app instance the event loop created;
    // its events arrive as user events through the proxy.
    let (menu, menu_ids) = build_menu();
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(event);
    }));

    let mut app = App {
        context: RenderContext::new(),
        renderers: vec![],
        state: RenderState::Suspended(None),
        scene: Scene::new(),
        font_cx: FontContext::new(),
        layout_cx: LayoutContext::new(),
        model: Model {
            doc,
            selection: None,
            collapse: raw::Collapse::default(),
            scroll: 0.0,
        },
        doc_path,
        menu,
        menu_ids,
        reducer: WindowEventReducer::default(),
    };

    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");
}

struct Model {
    doc: raw::Document,
    selection: Option<raw::Selection>,
    collapse: raw::Collapse,
    /// Vertical document scroll offset in logical pixels, so the
    /// position survives moving between monitor scales. May exceed
    /// the current maximum after a resize: placement clamps
    /// effectively, so a transient shrink-and-grow restores the
    /// position; scrolling collapses it to the clamped reality.
    scroll: f64,
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
    /// How far the document can scroll given this frame's content and
    /// viewport; dispatch clamps against it.
    max_scroll: f64,
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

    /// Scrolls the document, clamped to the frame's content.
    fn scroll_document(
        &mut self,
        update: &ui_events::pointer::PointerScrollEvent,
        scale: f64,
        viewport: f64,
        max_scroll: f64,
    ) -> bool {
        let line = 40.0 * scale;
        let delta = update.delta.to_pixel_delta(
            PhysicalPosition { x: line, y: line },
            PhysicalPosition {
                x: viewport,
                y: viewport,
            },
        );
        // ScrollDelta documents positive Y as viewport-down, but
        // ui-events-winit passes winit deltas through raw, where
        // positive Y is scroll-up; subtract to match reality. Stepping
        // from the clamped position keeps the first tick responsive
        // when a resize left the stored offset out of bounds.
        let next =
            (self.model.scroll.clamp(0.0, max_scroll) - delta.y / scale).clamp(0.0, max_scroll);
        (next != self.model.scroll) && {
            self.model.scroll = next;
            true
        }
    }

    fn title(&self) -> String {
        match &self.doc_path {
            Some(path) => format!("Progred — {}", path.display()),
            None => "Progred — untitled".into(),
        }
    }

    /// Save saves in place, or asks for a path when untitled; save-as
    /// always asks. Write-through editing means the graph is always
    /// current, so there is nothing to flush first. A cancelled dialog
    /// saves nothing.
    fn menu_save(&mut self, save_as: bool) {
        let in_place = (!save_as).then(|| self.doc_path.clone()).flatten();
        let target = in_place.or_else(|| dialog().set_file_name("untitled.progred").save_file());
        if let Some(path) = target {
            match store::save(&path, &self.model.doc) {
                Ok(()) => self.adopt_doc_path(path),
                Err(error) => {
                    eprintln!("failed to save {}: {error}", path.display());
                }
            }
        }
    }

    /// Open replaces the model. Selection, collapse overrides, and
    /// scroll are path-bound to the old document, so they reset with
    /// it.
    fn menu_open(&mut self) {
        if let Some(path) = dialog().pick_file() {
            match store::load(&path) {
                Ok(doc) => {
                    self.model = Model {
                        doc,
                        selection: None,
                        collapse: raw::Collapse::default(),
                        scroll: 0.0,
                    };
                    self.adopt_doc_path(path);
                }
                Err(error) => {
                    eprintln!("failed to open {}: {error}", path.display());
                }
            }
        }
    }

    fn adopt_doc_path(&mut self, path: PathBuf) {
        self.doc_path = Some(path);
        if let RenderState::Active { window, .. } = &self.state {
            window.set_title(&self.title());
            window.request_redraw();
        }
    }

    /// Space toggles the selected node's collapse override; a focused
    /// string editor claims the key first and types a space instead.
    fn collapse_key(&mut self, event: &KeyboardEvent) -> bool {
        event.state.is_down()
            && matches!(&event.key, Key::Character(c) if c.as_str() == " ")
            && match &self.model.selection {
                Some(selection) => raw::toggle_collapse(
                    &self.model.doc,
                    &mut self.model.collapse,
                    selection.path(),
                ),
                None => false,
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
            max_scroll: 0.0,
        };
        run_frame(
            &mut frame,
            &self.model,
            &mut self.font_cx,
            &mut self.layout_cx,
            scale,
            height as f64,
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
    viewport_height: f64,
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
        raw::Hooks {
            // The selection transition: re-selecting the same path
            // keeps its editor state, and a reported text click seeds
            // or advances the editor's caret — focus and cursor
            // placement are one event.
            select: Rc::new(move |app: &mut App, path, click| {
                if app.model.selection.as_ref().is_none_or(|s| s.path() != path) {
                    app.model.selection = Some(raw::Selection::edge(&app.model.doc, path));
                }
                if let Some(click) = click
                    && let Some(line) =
                        app.model.selection.as_mut().and_then(raw::Selection::edit_mut)
                {
                    line.refresh(&mut app.font_cx, &mut app.layout_cx, scale as f32);
                    line.pointer_down(
                        &mut app.font_cx,
                        &mut app.layout_cx,
                        click.point,
                        click.shift,
                        1,
                    );
                }
            }),
            toggle: Rc::new(|app: &mut App, path| {
                raw::toggle_collapse(&app.model.doc, &mut app.model.collapse, &path);
            }),
            edit: Rc::new(edit_ctx),
        },
    );
    let margin = 28.0 * scale;
    frame.max_scroll = ((body.extent.height() + 2.0 * margin - viewport_height) / scale).max(0.0);
    place_top_left(
        body,
        frame,
        Point::new(
            margin,
            margin - model.scroll.clamp(0.0, frame.max_scroll) * scale,
        ),
    );
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
