//! Window shell: winit + Vello plumbing around pure frame drawing.
//! `run_frame` writes to any puri `Canvas`; here it streams into vello.

mod completion;
mod conventions;
mod display;
mod document;
mod filter;
mod gid;
#[cfg(test)]
mod grap_examples;
mod graph_view;
mod history;
mod hover;
mod layout;
#[cfg(target_os = "macos")]
mod macos_menu;
mod menu;
mod model;
mod projection;
mod raw;
mod selection;
mod sources;
mod store;
#[cfg(test)]
mod test_values;

use crate::model::{Model, Selected, ViewFlags};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use parley::{FontContext, LayoutContext};
use progred_graph::{CellId, Step, Value};
use puri::draw::{Canvas, GlyphRun, Shape};
use puri::edit::{EditCtx, LineEditPointerDown, LineEditState, TextClipboard};
use puri::geometry::Placement;
use puri::handler::{Handler, HasHandler, ImeEvent};
use puri::text::TextCtx;
use puri_vello::VelloCanvas;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};
use ui_events::pointer::{PointerButton, PointerEvent};
use ui_events_winit::{WindowEventReducer, WindowEventTranslation};
use vello::kurbo::{Affine, Point, Rect, Size, Stroke, Vec2};
use vello::peniko::{Brush, Color};
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::{self, CurrentSurfaceTexture};
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{Ime, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

/// Everything arriving through the event-loop proxy.
enum UserEvent {
    #[cfg(target_os = "macos")]
    MacMenu(macos_menu::Event),
    Menu(menu::Selection),
    Discard(bool),
}

/// The action a discard confirmation gates. One at a time: requests
/// while a sheet is up are dropped.
enum AfterDiscard {
    New,
    Open,
    Quit,
}

enum RenderState {
    Active {
        surface: Box<RenderSurface<'static>>,
        valid_surface: bool,
        window: Arc<Window>,
    },
    Suspended(Option<Arc<Window>>),
}

/// The last rendered frame's dispatch outputs, retained until the
/// next redraw replaces them: the handler events feed, plus what the
/// shell's key fallbacks interpret. The user reacts to what was
/// presented, so its geometry is the honest hit-test target — and the
/// event path runs no pass at all.
/// The pasteboard type structural copies ride under, beside their
/// plain text; its PRESENCE is the structure/text distinction, so
/// text that merely spells Value JSON is never mistaken for a copy.
const CLIPBOARD_FORMAT: &str = "com.progred.value";

struct SystemTextClipboard;

impl TextClipboard for SystemTextClipboard {
    fn get_text(&mut self) -> Option<String> {
        use clipboard_rs::{Clipboard, ClipboardContext};
        ClipboardContext::new()
            .ok()
            .and_then(|cb| cb.get_text().ok())
    }

    fn set_text(&mut self, text: &str) {
        use clipboard_rs::{Clipboard, ClipboardContext};
        if let Ok(cb) = ClipboardContext::new() {
            cb.set_text(text.to_string()).ok();
        }
    }
}

struct Dispatch {
    handler: Handler<App>,
    descends: Vec<raw::Descend>,
    /// One nominal line height at the frame's scale — the quantum
    /// keyboard navigation reads rows with.
    line: f64,
    max_scroll: f64,
    max_scroll_x: f64,
    popup: Option<completion::Popup>,
}

struct App {
    context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    state: RenderState,
    scene: Scene,
    font_cx: FontContext,
    layout_cx: LayoutContext<Brush>,
    text_clipboard: SystemTextClipboard,
    text_cache: puri::text::TextCache,
    model: Model,
    /// Where the document lives; `None` is untitled until the first
    /// save asks for a path.
    doc_path: Option<PathBuf>,
    /// The notation's file-local binder table, surviving load → save
    /// so spellings round-trip; never part of the model, invisible
    /// in the document.
    binders: gid::Binders,
    #[cfg(target_os = "macos")]
    native_menu: macos_menu::Menu,
    menu: menu::State,
    /// Last pointer position, for anchoring pinch zoom.
    cursor: Point,
    /// The pointer position while it is inside the window. It is an
    /// input to placement's internal hover resolution.
    pointer: Option<Point>,
    /// Derived from pointer input and settled geometry. Kept outside
    /// the model for air hysteresis, pressed-gesture freezing, and the
    /// event-to-redraw handoff.
    hover: Option<Hovered>,
    /// A button is down: gestures keep the hover they began with, so
    /// hover resolution stands down until release.
    pressed: bool,
    /// Whether settled geometry has resolved `hover` for the
    /// next draw. Geometry-changing redraw sources clear it.
    hover_is_current: bool,
    /// The selection identity last scrolled into view — path AND
    /// variant, since Enter keeps the path while opening a pending —
    /// so reveal fires once per change and never fights manual
    /// scrolling.
    revealed: Option<(document::Path, std::mem::Discriminant<selection::Selection>)>,
    dispatch: Option<Dispatch>,
    reducer: WindowEventReducer,
    /// Routes the discard sheet's answer back into the loop.
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
    pending_discard: Option<AfterDiscard>,
}

fn menu_height(scale: f64) -> f64 {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = scale;
        0.0
    }
    #[cfg(target_os = "linux")]
    {
        menu::bar_height(scale)
    }
}

fn content_viewport(viewport: Size, scale: f64) -> Rect {
    Rect::new(
        0.0,
        menu_height(scale).min(viewport.height),
        viewport.width,
        viewport.height,
    )
}

fn graph_panel(viewport: Size, scale: f64) -> Rect {
    let content = content_viewport(viewport, scale);
    graph_view::panel(content.width(), content.height()) + Vec2::new(0.0, content.y0)
}

/// The position carried by any pointer translation, for cursor
/// tracking.
fn pointer_position(event: &PointerEvent) -> Option<Point> {
    match event {
        PointerEvent::Down(e) | PointerEvent::Up(e) => {
            Some(Point::new(e.state.position.x, e.state.position.y))
        }
        PointerEvent::Move(u) => Some(Point::new(u.current.position.x, u.current.position.y)),
        PointerEvent::Scroll(e) => Some(Point::new(e.state.position.x, e.state.position.y)),
        _ => None,
    }
}

/// The selection as a restorable edge path — pendings and graph
/// selections restore as nothing, being disposable.
fn edge_path(selection: &Option<Selected>) -> Option<document::Path> {
    match selection {
        Some(Selected::Tree(selection::Selection::Edge { path, .. })) => Some(path.clone()),
        _ => None,
    }
}

/// No modifiers at all — the gate for the bare editing keys.
fn plain(event: &KeyboardEvent) -> bool {
    !(event.modifiers.ctrl()
        || event.modifiers.meta()
        || event.modifiers.alt()
        || event.modifiers.shift())
}

fn dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("gid", &["gid"])
}

impl ApplicationHandler<UserEvent> for App {
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            #[cfg(target_os = "macos")]
            UserEvent::MacMenu(event) => {
                if let Some(selection) = self.native_menu.selection(&event) {
                    self.handle_menu_selection(event_loop, selection);
                }
            }
            UserEvent::Menu(selection) => self.handle_menu_selection(event_loop, selection),
            UserEvent::Discard(accepted) => {
                let pending = self.pending_discard.take();
                if accepted && let Some(then) = pending {
                    self.proceed(event_loop, then);
                }
            }
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // After launch, so winit cannot replace it (its own default
        // menu is disabled at loop construction).
        #[cfg(target_os = "macos")]
        self.native_menu.install();

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

        // Pinch zooms the graph toward the cursor; winit delivers it
        // outside the pointer stream the reducer covers.
        if let WindowEvent::PinchGesture { delta, .. } = &event
            && self.view_flags().graph
        {
            let size = window.inner_size();
            let panel = graph_panel(Size::new(size.width as f64, size.height as f64), scale);
            let anchor = if panel.contains(self.cursor) {
                self.cursor - panel.center()
            } else {
                Vec2::ZERO
            };
            self.model.graph.zoom_at(1.0 + delta, anchor, scale);
            self.hover_is_current = false;
            window.request_redraw();
            return;
        }

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
            if let Some(WindowEventTranslation::Pointer(pointer)) = &translation
                && let Some(position) = pointer_position(pointer)
            {
                self.cursor = position;
            }
            // Events dispatch into the retained frame's handler — a
            // pure function of the state it was built from, so it is
            // single-shot: a handled (mutating) event spends it and
            // the successor is minted immediately below. A genuinely
            // declined event leaves it standing only when no frame
            // input changed. Until the first redraw there is nothing
            // to dispatch into.
            if (ime.is_some() || translation.is_some())
                && let Some(dispatch) = self.dispatch.take()
            {
                let size = window.inner_size();
                let viewport = size.height as f64;
                let mut frame_input_changed = false;
                let handled = match (ime, translation) {
                    (Some(ime), _) => dispatch.handler.dispatch_ime(self, &ime),
                    // An open Linux menu owns the keyboard; otherwise
                    // its shared shortcuts are application commands.
                    // Remaining keys reach the editor first, except
                    // Cmd+V of STRUCTURE while a pending is open — the
                    // query must never eat Value JSON — then fall
                    // through to the structural commands.
                    (None, Some(WindowEventTranslation::Keyboard(key_event))) => {
                        self.menu_key(&key_event)
                            || self.pending_paste_key(&key_event)
                            || dispatch.handler.dispatch_key(self, &key_event)
                            || self.clipboard_key(&dispatch.descends, &key_event)
                            || self.graph_key(&key_event)
                            || self.delete_key(&dispatch.descends, &key_event)
                            || self.insert_key(&dispatch.descends, &dispatch.popup, &key_event)
                            || self.rename_key(&key_event)
                            || self.collapse_key(&key_event)
                            || match raw::step_selection(
                                &dispatch.descends,
                                self.model.tree_selection(),
                                dispatch.line,
                                &key_event,
                            ) {
                                Some(path) => {
                                    self.model.selection =
                                        Some(Selected::Tree(selection::selected_by_arrow(
                                            &self.model.sources(),
                                            path,
                                            &key_event,
                                        )));
                                    true
                                }
                                None => false,
                            }
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Down(button)))) => {
                        self.pointer =
                            Some(Point::new(button.state.position.x, button.state.position.y));
                        self.pressed = true;
                        dispatch.handler.dispatch_pointer_down(self, &button)
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Move(update)))) => {
                        // Pointer position is frame input. Unpressed
                        // motion remints even when no event handler
                        // consumes it; pressed gestures freeze hover
                        // while their ordinary drag handlers run.
                        self.pointer = Some(Point::new(
                            update.current.position.x,
                            update.current.position.y,
                        ));
                        frame_input_changed = !self.pressed;
                        dispatch.handler.dispatch_pointer_move(self, &update)
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Up(button)))) => {
                        self.pointer =
                            Some(Point::new(button.state.position.x, button.state.position.y));
                        self.pressed = false;
                        frame_input_changed = true;
                        dispatch.handler.dispatch_pointer_up(self, &button)
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Leave(_)))) => {
                        self.pointer = None;
                        self.pressed = false;
                        frame_input_changed = true;
                        false
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Scroll(update)))) => {
                        dispatch.handler.dispatch_scroll(self, &update)
                    }
                    _ => false,
                };
                if handled {
                    let model = &mut self.model;
                    if let Some(Selected::Tree(selection)) = &mut model.selection {
                        let before = model.doc.clone();
                        // True on the first write of the editor's
                        // life: the run's one step opens here.
                        if selection::write_through(&mut model.doc, &model.library, selection) {
                            let path = selection.path().to_vec();
                            model.history.record(before, Some(path));
                            self.refresh_title();
                        }
                    }
                }
                match frame_disposition(handled, frame_input_changed) {
                    FrameDisposition::Retain => self.dispatch = Some(dispatch),
                    FrameDisposition::Remint { reveal_selection } => {
                        let hover_changed = self.retain_dispatch(
                            scale,
                            Size::new(size.width as f64, viewport),
                            reveal_selection,
                        );
                        if handled || hover_changed {
                            window.request_redraw();
                        }
                    }
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                self.request_discard(event_loop, AfterDiscard::Quit);
            }

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
                    self.hover_is_current = false;
                    self.redraw();
                }
            }

            WindowEvent::ScaleFactorChanged { .. } => {
                self.hover_is_current = false;
                window.request_redraw();
            }

            // The hover is the pointer RELATIVE TO CONTENT, and a
            // moved window shifts that relation with no pointer event
            // — and no way to re-measure it (a title-bar drag carries
            // the mouse along; an OS-driven move doesn't; winit can't
            // say where the pointer now sits). The honest state is
            // unknown until the next move.
            WindowEvent::Moved(_) => {
                let changed = self.pointer.take().is_some() || self.hover.is_some();
                let window = match &self.state {
                    RenderState::Active { window, .. } => Some(window.clone()),
                    _ => None,
                };
                if changed && let Some(window) = window {
                    let size = window.inner_size();
                    let hover_changed = self.retain_dispatch(
                        window.scale_factor(),
                        Size::new(size.width as f64, size.height as f64),
                        false,
                    );
                    if hover_changed {
                        window.request_redraw();
                    }
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
    let (doc, binders) = match &doc_path {
        Some(path) if path.exists() => store::load(path).unwrap_or_else(|error| {
            eprintln!("failed to load {}: {error}", path.display());
            std::process::exit(1);
        }),
        // No path starts EMPTY — the sample lives in sample.gid now,
        // opened like any document.
        _ => (
            document::Document {
                root: None,
                cells: progred_graph::Cells::new(),
            },
            gid::Binders::new(),
        ),
    };

    let mut builder = EventLoop::<UserEvent>::with_user_event();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        builder.with_default_menu(false);
    }
    let event_loop = builder.build().expect("Couldn't create event loop");
    let proxy = event_loop.create_proxy();
    #[cfg(target_os = "macos")]
    let native_menu = macos_menu::Menu::new();
    #[cfg(target_os = "macos")]
    macos_menu::route_events(proxy.clone());

    let mut app = App {
        context: RenderContext::new(),
        renderers: vec![],
        state: RenderState::Suspended(None),
        scene: Scene::new(),
        font_cx: FontContext::new(),
        layout_cx: LayoutContext::new(),
        text_clipboard: SystemTextClipboard,
        text_cache: puri::text::TextCache::default(),
        model: Model {
            doc,
            selection: None,
            collapse: selection::Collapse::default(),
            names: conventions::Names::default(),
            library: conventions::library(),
            foreign: conventions::foreign_functions(),
            graph: graph_view::GraphView::default(),
            history: history::History::default(),
            view: ViewFlags::default(),
            scroll: 0.0,
            scroll_x: 0.0,
        },
        doc_path,
        binders,
        #[cfg(target_os = "macos")]
        native_menu,
        menu: menu::State::default(),
        cursor: Point::ZERO,
        pointer: None,
        hover: None,
        pressed: false,
        hover_is_current: false,
        revealed: None,
        dispatch: None,
        reducer: WindowEventReducer::default(),
        proxy,
        pending_discard: None,
    };

    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");
}

/// The app's one hover, the selection's shape: what the resting
/// pointer claims in whichever pane it rests over.
#[derive(Clone, Debug, PartialEq)]
enum Hovered {
    Tree(raw::Hovering),
    Graph(graph_view::GraphNode),
    #[cfg(target_os = "linux")]
    Menu(menu::Hover),
}

enum HoverHit {
    Tree(raw::HoverClaim),
    Graph(Option<graph_view::GraphNode>),
    #[cfg(target_os = "linux")]
    Menu(Option<menu::Hover>),
}

struct HoverResolver<'a> {
    current: &'a mut Option<Hovered>,
    pointer: Option<Point>,
    pressed: bool,
    reach: f64,
    hit: Option<HoverHit>,
}

impl HoverResolver<'_> {
    fn resolve(self) {
        *self.current = resolved_hover(
            self.current.as_ref(),
            self.hit,
            self.pointer,
            self.pressed,
            self.reach,
        );
    }
}

#[derive(Clone, Copy)]
enum FrameVisibility {
    Silent,
    Visible,
}

struct FrameDescription<'a> {
    model: &'a Model,
    view: ViewFlags,
    menu: menu::State,
    availability: menu::Availability,
    hover: Option<Hovered>,
    scale: f64,
    viewport: Size,
}

struct FrameResources<'a> {
    fonts: &'a mut FontContext,
    layouts: &'a mut LayoutContext<Brush>,
    text_cache: &'a mut puri::text::TextCache,
}

/// One read-only pass over the UI. Drawing is optional; every pass
/// still produces transient dispatch data and resolves pointer hover.
struct Frame<'a> {
    scene: Option<&'a mut Scene>,
    hover: HoverResolver<'a>,
    handler: Handler<App>,
    descends: Vec<raw::Descend>,
    /// How far the document can scroll given this frame's content and
    /// viewport; dispatch clamps against it.
    max_scroll: f64,
    max_scroll_x: f64,
    /// The pending row's completion popup, emitted during placement;
    /// drawn after the body and committed from at dispatch.
    popup: Option<completion::Popup>,
}

impl<'a> Frame<'a> {
    fn new(scene: Option<&'a mut Scene>, hover: HoverResolver<'a>) -> Self {
        Self {
            scene,
            hover,
            handler: Handler::new(),
            descends: Vec::new(),
            max_scroll: 0.0,
            max_scroll_x: 0.0,
            popup: None,
        }
    }

    fn finish(self, scale: f64) -> Dispatch {
        self.hover.resolve();
        Dispatch {
            handler: self.handler,
            descends: self.descends,
            line: 14.0 * scale,
            max_scroll: self.max_scroll,
            max_scroll_x: self.max_scroll_x,
            popup: self.popup,
        }
    }
}

impl hover::HasHover<raw::HoverClaim> for Frame<'_> {
    fn pointer(&self) -> Option<Point> {
        self.hover.pointer
    }

    fn claim_hover(&mut self, claim: raw::HoverClaim) {
        self.hover.hit = Some(HoverHit::Tree(claim));
    }
}

impl hover::HasHover<Option<graph_view::GraphNode>> for Frame<'_> {
    fn pointer(&self) -> Option<Point> {
        self.hover.pointer
    }

    fn claim_hover(&mut self, claim: Option<graph_view::GraphNode>) {
        self.hover.hit = Some(HoverHit::Graph(claim));
    }
}

#[cfg(target_os = "linux")]
impl hover::HasHover<Option<menu::Hover>> for Frame<'_> {
    fn pointer(&self) -> Option<Point> {
        self.hover.pointer
    }

    fn claim_hover(&mut self, claim: Option<menu::Hover>) {
        self.hover.hit = Some(HoverHit::Menu(claim));
    }
}

impl completion::HasPopup for Frame<'_> {
    fn popup(&mut self) -> &mut Option<completion::Popup> {
        &mut self.popup
    }
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

    fn clip(
        &mut self,
        shape: impl Into<Shape>,
        transform: Affine,
        content: impl FnOnce(&mut Self),
    ) {
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

fn resolved_hover(
    current: Option<&Hovered>,
    hit: Option<HoverHit>,
    pointer: Option<Point>,
    pressed: bool,
    reach: f64,
) -> Option<Hovered> {
    match (pressed, pointer) {
        (true, _) => current.cloned(),
        (false, None) => None,
        (false, Some(point)) => match hit {
            Some(HoverHit::Tree(raw::HoverClaim::Direct(hovering))) => hovering.map(Hovered::Tree),
            Some(HoverHit::Graph(node)) => node.map(Hovered::Graph),
            #[cfg(target_os = "linux")]
            Some(HoverHit::Menu(hover)) => hover.map(Hovered::Menu),
            Some(HoverHit::Tree(raw::HoverClaim::Air)) | None => {
                let tree = match current {
                    Some(Hovered::Tree(hovering)) => Some(hovering),
                    _ => None,
                };
                match raw::resolve_hover(raw::HoverClaim::Air, tree, point, reach) {
                    Some(next) => next.map(Hovered::Tree),
                    None => current.cloned(),
                }
            }
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FrameDisposition {
    Retain,
    Remint { reveal_selection: bool },
}

fn frame_disposition(handled: bool, frame_input_changed: bool) -> FrameDisposition {
    if handled {
        FrameDisposition::Remint {
            reveal_selection: true,
        }
    } else if frame_input_changed {
        FrameDisposition::Remint {
            reveal_selection: false,
        }
    } else {
        FrameDisposition::Retain
    }
}

impl App {
    /// Scrolls the document, clamped to the frame's content.
    fn scroll_document(
        &mut self,
        update: &ui_events::pointer::PointerScrollEvent,
        scale: f64,
        viewport: f64,
        max_scroll: f64,
        max_scroll_x: f64,
    ) -> bool {
        let line = 40.0 * scale;
        let delta = update.delta.to_pixel_delta(
            PhysicalPosition { x: line, y: line },
            PhysicalPosition {
                x: viewport,
                y: viewport,
            },
        );
        // ScrollDelta documents positive as viewport-down/right, but
        // ui-events-winit passes winit deltas through raw, where
        // positive is scroll-up/left; subtract to match reality.
        // Stepping from the clamped position keeps the first tick
        // responsive when a resize left the stored offset out of
        // bounds.
        let next =
            (self.model.scroll.clamp(0.0, max_scroll) - delta.y / scale).clamp(0.0, max_scroll);
        let next_x = (self.model.scroll_x.clamp(0.0, max_scroll_x) - delta.x / scale)
            .clamp(0.0, max_scroll_x);
        (next != self.model.scroll || next_x != self.model.scroll_x) && {
            self.model.scroll = next;
            self.model.scroll_x = next_x;
            true
        }
    }

    fn title(&self) -> String {
        let dirty = if self.model.history.dirty() {
            " •"
        } else {
            ""
        };
        match &self.doc_path {
            Some(path) => format!("Progred — {}{dirty}", path.display()),
            None => format!("Progred — untitled{dirty}"),
        }
    }

    fn refresh_title(&self) {
        if let RenderState::Active { window, .. } = &self.state {
            window.set_title(&self.title());
        }
    }

    /// Menu enablement follows the model: gray what can't act. Save
    /// stays live for untitled documents — it defers to the save
    /// panel, per platform convention.
    fn sync_menus(&self) {
        #[cfg(target_os = "macos")]
        self.native_menu
            .sync(self.menu_availability(), self.model.view);
    }

    fn menu_availability(&self) -> menu::Availability {
        menu::Availability {
            save: self.model.history.dirty() || self.doc_path.is_none(),
            undo: self.model.history.can_undo(),
            redo: self.model.history.can_redo(),
        }
    }

    fn handle_menu_selection(&mut self, event_loop: &ActiveEventLoop, selection: menu::Selection) {
        match selection {
            menu::Selection::New => self.request_discard(event_loop, AfterDiscard::New),
            menu::Selection::Open => self.request_discard(event_loop, AfterDiscard::Open),
            menu::Selection::Save => self.menu_save(false),
            menu::Selection::SaveAs => self.menu_save(true),
            menu::Selection::Quit => self.request_discard(event_loop, AfterDiscard::Quit),
            menu::Selection::Undo => self.step_history(true),
            menu::Selection::Redo => self.step_history(false),
            menu::Selection::Raw => self.model.view.raw = !self.model.view.raw,
            menu::Selection::Graph => self.model.view.graph = !self.model.view.graph,
        }
        if matches!(selection, menu::Selection::Raw | menu::Selection::Graph)
            && let RenderState::Active { window, .. } = &self.state
        {
            self.hover_is_current = false;
            window.request_redraw();
        }
    }

    fn choose_menu(&mut self, selection: menu::Selection) {
        self.menu.close();
        let _ = self.proxy.send_event(UserEvent::Menu(selection));
    }

    fn menu_key(&mut self, event: &KeyboardEvent) -> bool {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = event;
            false
        }
        #[cfg(target_os = "linux")]
        {
            let open = self.menu.open().is_some();
            if open
                && event.state.is_down()
                && plain(event)
                && matches!(event.key, Key::Named(NamedKey::Escape))
            {
                self.menu.close()
            } else {
                menu::shortcut(event)
                    .filter(|selection| self.menu_availability().enabled(*selection))
                    .is_some_and(|selection| {
                        self.choose_menu(selection);
                        true
                    })
                    || self.menu.captures_key(event)
            }
        }
    }

    /// Undo or redo one step, restoring the snapshot's document and
    /// selection; the displaced state crosses to the other stack.
    fn step_history(&mut self, back: bool) {
        let current = self.model.doc.clone();
        let selection = edge_path(&self.model.selection);
        let restored = if back {
            self.model.history.undo(current, selection)
        } else {
            self.model.history.redo(current, selection)
        };
        if let Some((doc, restore)) = restored {
            self.model.doc = doc;
            // One slot: restoring (or clearing) the tree selection
            // also drops any graph selection, which may reference
            // content the restored document no longer has.
            self.model.selection = restore
                .map(|path| Selected::Tree(selection::Selection::edge(&self.model.sources(), path)));
            self.refresh_title();
            if let RenderState::Active { window, .. } = &self.state {
                let window = window.clone();
                let size = window.inner_size();
                let scale = window.scale_factor();
                self.retain_dispatch(
                    scale,
                    Size::new(size.width as f64, size.height as f64),
                    true,
                );
                window.request_redraw();
            }
        }
    }

    /// Unsaved changes gate for the document-replacing commands.
    /// Parented to the window, the dialog is a real NSAlert sheet;
    /// parentless, rfd falls back to a CFUserNotification panel that
    /// arrives unfocused (a click to focus, then a click to answer)
    /// and warns on the console.
    /// Gate an action on unsaved changes: a clean document proceeds
    /// immediately; a dirty one presents the standard window sheet
    /// and continues through a [`UserEvent::Discard`] when it
    /// resolves. Async is the one rfd path that presents natively on
    /// macOS (begin-sheet with a completion; the sync API falls back
    /// to CFUserNotification parentless and a sheet-plus-modal-loop
    /// double-present parented). The sheet's completion lands on the
    /// main run loop, so the answer is awaited on a throwaway thread
    /// and routed back through the proxy — blocking here would
    /// deadlock the loop the sheet needs.
    fn request_discard(&mut self, event_loop: &ActiveEventLoop, then: AfterDiscard) {
        if !self.model.history.dirty() {
            self.proceed(event_loop, then);
            return;
        }
        if self.pending_discard.is_some() {
            return;
        }
        let RenderState::Active { window, .. } = &self.state else {
            return;
        };
        self.pending_discard = Some(then);
        // rfd reports a custom button by its label; one spelling.
        const DISCARD: &str = "Discard";
        let sheet = rfd::AsyncMessageDialog::new()
            .set_title("Discard unsaved changes?")
            .set_buttons(rfd::MessageButtons::OkCancelCustom(
                DISCARD.to_string(),
                "Cancel".to_string(),
            ))
            .set_parent(window.as_ref())
            .show();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let accepted = matches!(
                pollster::block_on(sheet),
                rfd::MessageDialogResult::Custom(choice) if choice == DISCARD
            );
            let _ = proxy.send_event(UserEvent::Discard(accepted));
        });
    }

    /// The action a confirmed (or unneeded) discard proceeds to.
    fn proceed(&mut self, event_loop: &ActiveEventLoop, then: AfterDiscard) {
        match then {
            AfterDiscard::New => self.adopt_model(
                document::Document {
                    root: None,
                    cells: progred_graph::Cells::new(),
                },
                None,
                gid::Binders::new(),
            ),
            AfterDiscard::Open => {
                if let Some(path) = dialog().pick_file() {
                    match store::load(&path) {
                        Ok((doc, binders)) => self.adopt_model(doc, Some(path), binders),
                        Err(error) => {
                            eprintln!("failed to open {}: {error}", path.display());
                        }
                    }
                }
            }
            AfterDiscard::Quit => event_loop.exit(),
        }
    }

    /// Save saves in place, or asks for a path when untitled; save-as
    /// always asks. Write-through editing means the graph is always
    /// current, so there is nothing to flush first. A cancelled dialog
    /// saves nothing.
    fn menu_save(&mut self, save_as: bool) {
        let in_place = (!save_as).then(|| self.doc_path.clone()).flatten();
        let target = in_place.or_else(|| dialog().set_file_name("untitled.gid").save_file());
        if let Some(path) = target {
            match store::save(&path, &self.model.doc, &self.binders) {
                Ok(()) => {
                    self.model.history.mark_saved();
                    // A run must not straddle the save mark, or edits
                    // after it would coalesce into a pre-save step.
                    selection::break_edit_run(self.model.tree_selection_mut());
                    self.adopt_doc_path(path);
                }
                Err(error) => {
                    eprintln!("failed to save {}: {error}", path.display());
                }
            }
        }
    }

    /// Replaces the model wholesale for New and Open. Selection,
    /// collapse overrides, scroll, and history are bound to the old
    /// document and reset with it. Mints the successor dispatch
    /// immediately, as every mutation site does: the retained handler
    /// was built from the old document, and its dispatches must not
    /// run against the new model.
    fn adopt_model(&mut self, doc: document::Document, path: Option<PathBuf>, binders: gid::Binders) {
        self.binders = binders;
        let view = self.model.view;
        self.model = Model {
            doc,
            selection: None,
            collapse: selection::Collapse::default(),
            names: self.model.names.clone(),
            library: conventions::library(),
            foreign: conventions::foreign_functions(),
            graph: graph_view::GraphView::default(),
            history: history::History::default(),
            view,
            scroll: 0.0,
            scroll_x: 0.0,
        };
        self.hover = None;
        self.doc_path = path;
        self.revealed = None;
        if let RenderState::Active { window, .. } = &self.state {
            let window = window.clone();
            window.set_title(&self.title());
            let size = window.inner_size();
            self.retain_dispatch(
                window.scale_factor(),
                Size::new(size.width as f64, size.height as f64),
                false,
            );
            window.request_redraw();
        }
    }

    fn adopt_doc_path(&mut self, path: PathBuf) {
        self.doc_path = Some(path);
        if let RenderState::Active { window, .. } = &self.state {
            window.set_title(&self.title());
            window.request_redraw();
        }
    }

    /// Scroll-to-reveal, computed from the freshly retained dispatch
    /// pass BEFORE anything draws, so the reveal lands in the next
    /// presented frame with no corrective flash. Fires once per
    /// selection-identity change (path AND variant — Enter keeps the
    /// path while opening a pending), so it never fights manual
    /// scrolling. The target is the popup anchor while pending — it
    /// marks the authoring row — else the selection's rect.
    fn reveal_selection(&mut self, dispatch: &Dispatch, scale: f64, viewport: Size) -> bool {
        let reveal = self
            .model
            .tree_selection()
            .map(|s| (s.path().to_vec(), std::mem::discriminant(s)));
        if reveal == self.revealed {
            false
        } else {
            self.revealed = reveal.clone();
            let target = dispatch
                .popup
                .as_ref()
                .map(|popup| popup.anchor)
                .or_else(|| {
                    reveal.as_ref().and_then(|(path, _)| {
                        dispatch
                            .descends
                            .iter()
                            .find(|descend| &descend.path == path)
                            .map(|descend| descend.rect)
                    })
                });
            target.is_some_and(|rect| {
                let before = (self.model.scroll, self.model.scroll_x);
                let pad = 12.0 * scale;
                let content = content_viewport(viewport, scale);
                let mut scroll = self.model.scroll;
                // The pad is the landing margin, not the trigger: fully
                // visible rects are left alone, so a click near an edge
                // doesn't nudge.
                if rect.y1 > content.y1 {
                    scroll += (rect.y1 + pad - content.y1) / scale;
                }
                // Checked against the adjusted position, so when the rect
                // is taller than the viewport the top wins.
                let top = rect.y0 - (scroll - self.model.scroll) * scale;
                if top < content.y0 {
                    scroll += (top - pad - content.y0) / scale;
                }
                self.model.scroll = scroll.clamp(0.0, dispatch.max_scroll);
                // The same chase horizontally, against the width the
                // graph panel leaves visible.
                let visible = if self.view_flags().graph {
                    graph_panel(viewport, scale).x0
                } else {
                    viewport.width
                };
                let mut scroll_x = self.model.scroll_x;
                if rect.x1 > visible {
                    scroll_x += (rect.x1 + pad - visible) / scale;
                }
                let left = rect.x0 - (scroll_x - self.model.scroll_x) * scale;
                if left < 0.0 {
                    scroll_x += (left - pad) / scale;
                }
                self.model.scroll_x = scroll_x.clamp(0.0, dispatch.max_scroll_x);
                (self.model.scroll, self.model.scroll_x) != before
            })
        }
    }

    fn view_flags(&self) -> ViewFlags {
        self.model.view
    }

    fn build_frame(&mut self, visibility: FrameVisibility, scale: f64, viewport: Size) -> Dispatch {
        let view = self.view_flags();
        let availability = self.menu_availability();
        let presented_hover = self.hover.clone();
        let scene = match visibility {
            FrameVisibility::Silent => None,
            FrameVisibility::Visible => Some(&mut self.scene),
        };
        let description = FrameDescription {
            model: &self.model,
            view,
            menu: self.menu,
            availability,
            hover: presented_hover,
            scale,
            viewport,
        };
        let resources = FrameResources {
            fonts: &mut self.font_cx,
            layouts: &mut self.layout_cx,
            text_cache: &mut self.text_cache,
        };
        let hover = HoverResolver {
            current: &mut self.hover,
            pointer: self.pointer,
            pressed: self.pressed,
            reach: 8.0 * scale,
            hit: None,
        };
        let mut frame = Frame::new(scene, hover);
        run_frame(&mut frame, description, resources);
        frame.finish(scale)
    }

    /// Mint dispatch data from the final state of a transition. A
    /// silent pass supplies reveal geometry and resolves hover;
    /// scrolling to reveal changes geometry and earns one rebuild.
    fn retain_dispatch(&mut self, scale: f64, viewport: Size, reveal_selection: bool) -> bool {
        let before = self.hover.clone();
        let mut dispatch = self.build_frame(FrameVisibility::Silent, scale, viewport);
        if reveal_selection && self.reveal_selection(&dispatch, scale, viewport) {
            dispatch = self.build_frame(FrameVisibility::Silent, scale, viewport);
        }
        let hover_changed = self.hover != before;
        self.dispatch = Some(dispatch);
        self.hover_is_current = true;
        hover_changed
    }

    /// Graph-view keys: Delete detaches the selected node — the
    /// cell's whole entry removed and every link to it unlinked, or
    /// the root emptied. The one selection slot means this and the
    /// document delete below can never both match; Escape falls
    /// through to the universal clear in `insert_key`.
    fn graph_key(&mut self, event: &KeyboardEvent) -> bool {
        event.state.is_down()
            && plain(event)
            && matches!(
                &event.key,
                Key::Named(NamedKey::Backspace | NamedKey::Delete)
            )
            && match self.model.graph_selection() {
                Some(selection) => {
                    let selection = *selection;
                    let before = self.model.doc.clone();
                    if graph_view::delete_selection(&mut self.model.doc, &selection) {
                        self.model.history.record(before, None);
                        self.refresh_title();
                    }
                    self.model.selection = None;
                    true
                }
                None => false,
            }
    }

    /// Backspace or Delete removes the selected edge — a focused atom
    /// editor claims the keys while it has text and declines on an
    /// empty buffer, so emptying a string then backspacing again
    /// deletes the element. Selection lands on the next sibling, else
    /// the previous, else the parent.
    fn delete_key(&mut self, descends: &[raw::Descend], event: &KeyboardEvent) -> bool {
        event.state.is_down()
            && plain(event)
            && matches!(
                &event.key,
                Key::Named(NamedKey::Backspace | NamedKey::Delete)
            )
            && self.delete_selected_edge(descends)
    }

    /// Deletes the selected edge and lands the selection on a
    /// survivor — Backspace/Delete's action, and cut's second half.
    fn delete_selected_edge(&mut self, descends: &[raw::Descend]) -> bool {
        match &self.model.selection {
            // Only a real edge deletes; a pending's Backspace is its
            // cancel, handled by insert_key.
            Some(Selected::Tree(selection::Selection::Edge { path, recorded, .. })) => {
                let path = path.clone();
                // Backspacing through the value and once more to
                // delete the edge is one gesture: when this edge has
                // the open run, its frame (pre-run document, edge
                // intact) already covers the deletion.
                let covered = *recorded;
                let before = self.model.doc.clone();
                selection::delete_edge(&mut self.model.doc, &self.model.library, &path) && {
                    if !covered {
                        self.model.history.record(before, Some(path.clone()));
                        self.refresh_title();
                    }
                    let next = raw::selection_after_delete(descends, &path);
                    self.model.selection = Some(Selected::Tree(selection::Selection::edge(
                        &self.model.sources(),
                        next,
                    )));
                    true
                }
            }
            _ => false,
        }
    }

    /// The chosen entry's action — from the frame's popup, else the
    /// query's inferred atom.
    fn chosen_action(
        popup: &Option<completion::Popup>,
        query: &LineEditState,
        choice: usize,
        labels: bool,
    ) -> completion::EntryAction {
        popup
            .as_ref()
            .and_then(|p| p.entries.get(choice.min(p.entries.len().saturating_sub(1))))
            .map(|entry| entry.action.clone())
            .unwrap_or_else(|| {
                if labels {
                    completion::EntryAction::NewLabel(query.text().to_string())
                } else {
                    completion::EntryAction::Value(selection::resolve_query(query.text()))
                }
            })
    }

    /// Commits a pointed-at value into the open pending — the
    /// command-click gesture. A value-stage pending commits and
    /// selects the edge; a label stage advances to its value stage.
    /// False when nothing is pending — or when the picked value
    /// cannot label (a list, a record, a blob) at the label stage —
    /// so the click falls through rather than spending the pending.
    fn pick_identity(&mut self, id: Value) -> bool {
        if matches!(
            self.model.selection,
            Some(Selected::Tree(selection::Selection::PendingEdge { .. }))
        ) && id.as_cell().is_none()
        {
            return false;
        }
        match self.model.selection.take() {
            Some(Selected::Tree(selection::Selection::Pending { path, .. })) => {
                self.commit_value(path, &completion::EntryAction::Value(id));
                true
            }
            Some(Selected::Tree(selection::Selection::PendingEdge {
                parent, replacing, ..
            })) => {
                self.commit_label(parent, replacing, &completion::EntryAction::Value(id));
                true
            }
            selection => {
                self.model.selection = selection;
                false
            }
        }
    }

    /// Commits the pending value stage — one undo step — and selects
    /// the edge it wrote.
    fn commit_value(&mut self, path: document::Path, action: &completion::EntryAction) {
        let before = self.model.doc.clone();
        if completion::commit_pending(&mut self.model.doc, &self.model.library, &path, action) {
            self.model.history.record(before, None);
            self.refresh_title();
        }
        self.model.selection = Some(Selected::Tree(selection::Selection::edge(
            &self.model.sources(),
            path,
        )));
    }

    /// A resolved label advances the pending edge to its value stage —
    /// or selects the existing field when the label is taken (rename
    /// included: a taken label never clobbers its field, selection
    /// communicates it, and replacing it means deleting it first).
    /// A free-text label persists its newly named cell before the
    /// value stage; a bare-cell choice has nothing to persist. A
    /// rename re-keys the field and creates its label cell in one
    /// history step, the value carried.
    fn commit_label(
        &mut self,
        parent: document::Path,
        replacing: Option<CellId>,
        action: &completion::EntryAction,
    ) {
        let Some((label, created)) = completion::resolve_label(action) else {
            return;
        };
        let mut path = parent.clone();
        path.push(Step::Key(label));
        if self.model.sources().resolve(&path).is_some() {
            self.model.selection = Some(Selected::Tree(selection::Selection::edge(
                &self.model.sources(),
                path,
            )));
            return;
        }
        match replacing {
            Some(old) => {
                let before = self.model.doc.clone();
                if let Some((cell, value)) = &created {
                    self.model.doc.cells.set_value(*cell, value.clone());
                }
                let renamed = selection::rename_field(
                    &mut self.model.doc,
                    &self.model.library,
                    &parent,
                    &old,
                    label,
                );
                if renamed {
                    self.model.history.record(before, None);
                    self.refresh_title();
                } else {
                    if let Some((cell, _)) = created {
                        self.model.doc.cells.clear_value(cell);
                    }
                    // The rename could not land; back to the field.
                    path = parent;
                    path.push(Step::Key(old));
                }
                self.model.selection = Some(Selected::Tree(selection::Selection::edge(
                    &self.model.sources(),
                    path,
                )));
            }
            None => {
                if let Some((cell, value)) = created {
                    let before = self.model.doc.clone();
                    self.model.doc.cells.set_value(cell, value);
                    self.model.history.record(before, None);
                    self.refresh_title();
                }
                self.model.selection = Some(Selected::Tree(selection::pending_value(path)));
            }
        }
    }

    /// Structural copy/paste, the shell's fallback: a focused text
    /// editor's own clipboard handling wins by dispatch order, so
    /// these fire on cell, list, and graph selections. Deliberately
    /// NOT menu items — muda accelerators intercept ahead of key
    /// dispatch, which would take Cmd+C/V away from text editing.
    fn clipboard_key(&mut self, descends: &[raw::Descend], event: &KeyboardEvent) -> bool {
        if !event.state.is_down() || !raw::command(&event.modifiers) {
            return false;
        }
        let Key::Character(c) = &event.key else {
            return false;
        };
        match c.to_lowercase().as_str() {
            "c" => self.copy_selection(),
            "x" => self.copy_selection() && self.delete_selected_edge(descends),
            "v" => self.paste_clipboard(),
            _ => false,
        }
    }

    /// Copies the selected value — SHALLOW: a link is its identity
    /// alone, no cell values travel; the value carries its own inline
    /// structure. Graph selections copy their node's value.
    fn copy_selection(&self) -> bool {
        use clipboard_rs::{Clipboard, ClipboardContext};
        let sources = self.model.sources();
        let value = match &self.model.selection {
            Some(Selected::Tree(selection)) => sources.resolve(selection.path()).cloned(),
            Some(Selected::Graph(graph_view::GraphSelection::Node(node))) => {
                graph_view::node_value(&self.model.doc, node)
            }
            None => None,
        };
        let Some(value) = value else {
            return false;
        };
        let (text, structural) = selection::to_clipboard(&value);
        ClipboardContext::new()
            .and_then(|cb| {
                if structural {
                    // Both representations: the private format says
                    // "structure", the text reads anywhere.
                    cb.set(vec![
                        clipboard_rs::ClipboardContent::Other(
                            CLIPBOARD_FORMAT.to_string(),
                            text.clone().into_bytes(),
                        ),
                        clipboard_rs::ClipboardContent::Text(text),
                    ])
                } else {
                    cb.set_text(text)
                }
            })
            .is_ok()
    }

    /// The private format's payload, when the clipboard carries one.
    fn clipboard_structure(&self) -> Option<Value> {
        use clipboard_rs::{Clipboard, ClipboardContext};
        let bytes = ClipboardContext::new()
            .ok()
            .and_then(|cb| cb.get_buffer(CLIPBOARD_FORMAT).ok())?;
        selection::from_structure(&bytes)
    }

    /// Cmd+V while a pending is open and the clipboard CARRIES
    /// STRUCTURE — the private format, not a text shape — commits the
    /// value into the pending, ahead of the focused query's own text
    /// paste. Everything else keeps the text path: pasting "hi",
    /// 0xff, or even text that happens to spell Value JSON lands in
    /// the query as characters. Claims the chord even when the pick
    /// declines (the label stage takes only what can label).
    fn pending_paste_key(&mut self, event: &KeyboardEvent) -> bool {
        if !event.state.is_down() || !raw::command(&event.modifiers) {
            return false;
        }
        if !matches!(&event.key, Key::Character(c) if c.to_lowercase().as_str() == "v") {
            return false;
        }
        if !matches!(
            self.model.selection,
            Some(Selected::Tree(
                selection::Selection::Pending { .. } | selection::Selection::PendingEdge { .. }
            ))
        ) {
            return false;
        }
        let Some(value) = self.clipboard_structure() else {
            return false;
        };
        self.pick_identity(value);
        true
    }

    /// Pastes the clipboard's value — the private format's structure
    /// when it carries one, else the text's query reading: into an
    /// open pending first (the label stage narrows to atoms through
    /// the pick), else over the selected edge — one undo step, the
    /// selection remounted so a pasted atom gets its editor.
    fn paste_clipboard(&mut self) -> bool {
        use clipboard_rs::{Clipboard, ClipboardContext};
        let value = match self.clipboard_structure() {
            Some(value) => value,
            None => {
                let Some(text) = ClipboardContext::new()
                    .ok()
                    .and_then(|cb| cb.get_text().ok())
                else {
                    return false;
                };
                if text.is_empty() {
                    return false;
                }
                selection::from_clipboard(&text)
            }
        };
        if self.pick_identity(value.clone()) {
            return true;
        }
        let Some(Selected::Tree(selection::Selection::Edge { path, .. })) = &self.model.selection else {
            return false;
        };
        let path = path.clone();
        // Idempotent pastes stay off the undo stack, as write_through
        // keeps no-op rewrites off it.
        if self.model.sources().resolve(&path) == Some(&value) {
            return true;
        }
        let before = self.model.doc.clone();
        if selection::set_value(&mut self.model.doc, &self.model.library, &path, value) {
            self.model.history.record(before, Some(path.clone()));
            self.refresh_title();
            self.model.selection = Some(Selected::Tree(selection::Selection::edge(
                &self.model.sources(),
                path,
            )));
            true
        } else {
            false
        }
    }

    /// Enter advances a pending stage or begins one (the chains live
    /// in raw). Plain Enter is a new peer BESIDE the selection: a
    /// sibling element in a list (Shift+Enter before), a new field on
    /// the parent record otherwise; the root has nothing beside it
    /// and takes the field on itself. The command chord authors
    /// WITHIN the selection: a field on the selected cell, an
    /// appended element on a list (with Shift, at the front). Labels
    /// author first, then values; list elements are one-stage value
    /// pendings, the projection minting the position.
    /// On an empty document Enter begins the root value. Escape
    /// clears the selection from anywhere, discarding any pending
    /// with the graph untouched; Backspace on an empty query cancels
    /// a pending back to its anchor instead, keeping the keyboard
    /// flow.
    fn insert_key(
        &mut self,
        descends: &[raw::Descend],
        popup: &Option<completion::Popup>,
        event: &KeyboardEvent,
    ) -> bool {
        event.state.is_down()
            && match &event.key {
                // While pending, plain vertical arrows drive the popup
                // choice; chorded arrows stay structure keys.
                Key::Named(direction @ (NamedKey::ArrowUp | NamedKey::ArrowDown))
                    if !raw::command(&event.modifiers) =>
                {
                    match &mut self.model.selection {
                        Some(Selected::Tree(
                            selection::Selection::Pending { choice, .. }
                            | selection::Selection::PendingEdge { choice, .. },
                        )) => {
                            let len = popup.as_ref().map(|p| p.entries.len()).unwrap_or(0);
                            *choice = match direction {
                                NamedKey::ArrowUp => choice.saturating_sub(1),
                                _ => (*choice + 1).min(len.saturating_sub(1)),
                            };
                            true
                        }
                        _ => false,
                    }
                }
                Key::Named(NamedKey::Enter) => match self.model.selection.take() {
                    Some(Selected::Tree(selection::Selection::Pending {
                        path,
                        query,
                        choice,
                    })) => {
                        let action = Self::chosen_action(popup, &query, choice, false);
                        self.commit_value(path, &action);
                        true
                    }
                    Some(Selected::Tree(selection::Selection::PendingEdge {
                        parent,
                        query,
                        choice,
                        replacing,
                    })) => {
                        let action = Self::chosen_action(popup, &query, choice, true);
                        self.commit_label(parent, replacing, &action);
                        true
                    }
                    selection => {
                        // Only a tree selection anchors authoring; a
                        // graph selection has no path to author at.
                        let tree = match &selection {
                            Some(Selected::Tree(current)) => Some(current),
                            _ => None,
                        };
                        let sources = self.model.sources();
                        let shift = event.modifiers.shift();
                        let started = match tree {
                            Some(current) if raw::command(&event.modifiers) => {
                                selection::pending_insert(&sources, current.path(), shift)
                            }
                            Some(current) => selection::pending_enter(&sources, current.path(), shift),
                            None => selection::pending_root(&sources),
                        };
                        let began = started.is_some();
                        self.model.selection = started.map(Selected::Tree).or(selection);
                        began
                    }
                },
                Key::Named(NamedKey::Escape) => self.model.selection.take().is_some(),
                Key::Named(NamedKey::Backspace) => {
                    match &self.model.selection {
                        Some(Selected::Tree(selection::Selection::Pending { path, .. })) => {
                            let back = raw::selection_after_delete(descends, path);
                            // Cancelling the empty document's root
                            // pending deselects — reselecting it
                            // would pend again.
                            self.model.selection = (!(back.is_empty()
                                && self.model.doc.root.is_none()))
                            .then(|| {
                                Selected::Tree(selection::Selection::edge(&self.model.sources(), back))
                            });
                            true
                        }
                        Some(Selected::Tree(selection::Selection::PendingEdge {
                            parent,
                            replacing,
                            ..
                        })) => {
                            // A cancelled rename returns to its field;
                            // a cancelled new field to the record.
                            let mut back = parent.clone();
                            if let Some(old) = replacing {
                                back.push(Step::Key(*old));
                            }
                            self.model.selection = Some(Selected::Tree(selection::Selection::edge(
                                &self.model.sources(),
                                back,
                            )));
                            true
                        }
                        _ => false,
                    }
                }
                _ => false,
            }
    }

    /// Cmd+L re-opens the selected field's label as its seeded rename
    /// query — the keyboard route to what clicking the label does. The
    /// popup opens only on this explicit ask, never during navigation.
    /// (Cmd+R belongs to the Raw view toggle.)
    fn rename_key(&mut self, event: &KeyboardEvent) -> bool {
        event.state.is_down()
            && raw::command(&event.modifiers)
            && matches!(&event.key, Key::Character(c) if c.to_lowercase().as_str() == "l")
            && match &self.model.selection {
                Some(Selected::Tree(selection::Selection::Edge { path, .. })) => {
                    let path = path.clone();
                    match selection::pending_rename(&self.model.sources(), &path) {
                        Some(pending) => {
                            self.model.selection = Some(Selected::Tree(pending));
                            true
                        }
                        None => false,
                    }
                }
                _ => false,
            }
    }

    /// Space toggles the selection's collapse override, and Cmd+Up /
    /// Cmd+Down close and open it — the fold axis of the keyboard's
    /// third dimension, under the same keys that walk the rows. A
    /// focused string editor claims Space first and types instead.
    fn collapse_key(&mut self, event: &KeyboardEvent) -> bool {
        if !event.state.is_down() {
            return false;
        }
        let set = match &event.key {
            Key::Character(c) if c.as_str() == " " => None,
            Key::Named(NamedKey::ArrowUp) if raw::command(&event.modifiers) => Some(true),
            Key::Named(NamedKey::ArrowDown) if raw::command(&event.modifiers) => Some(false),
            _ => return false,
        };
        let Some(Selected::Tree(selection::Selection::Edge { path, .. })) = &self.model.selection else {
            return false;
        };
        let path = path.clone();
        let sources = sources::Sources {
            doc: &self.model.doc,
            library: &self.model.library,
        };
        match set {
            None => selection::toggle_collapse(&sources, &mut self.model.collapse, &path),
            Some(closed) => selection::set_collapse(&sources, &mut self.model.collapse, &path, closed),
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

        // Advance the force simulation while the graph is open; the
        // continuous redraw request below keeps it animating.
        self.sync_menus();
        let view = self.view_flags();
        if view.graph {
            self.model.graph.step(&self.model.doc);
            self.hover_is_current = false;
        }
        let viewport = Size::new(width as f64, height as f64);
        if !self.pressed && !self.hover_is_current {
            self.build_frame(FrameVisibility::Silent, scale, viewport);
        }
        self.scene.reset();
        let before = self.hover.clone();
        let dispatch = self.build_frame(FrameVisibility::Visible, scale, viewport);
        // Recover from any geometry invalidation the shell failed to mark.
        let hover_changed = self.hover != before;
        self.dispatch = Some(dispatch);
        self.hover_is_current = true;

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

        if hover_changed || (view.graph && self.model.graph.hot()) {
            window.request_redraw();
        }
    }
}

fn run_frame(
    frame: &mut Frame<'_>,
    description: FrameDescription<'_>,
    resources: FrameResources<'_>,
) {
    let FrameDescription {
        model,
        view,
        menu,
        availability,
        hover,
        scale,
        viewport,
    } = description;
    let FrameResources {
        fonts: font_cx,
        layouts: layout_cx,
        text_cache,
    } = resources;
    let (viewport_width, viewport_height) = (viewport.width, viewport.height);
    // Empty space deselects — the one slot, whichever pane filled it.
    // Registered before the content places, so the descend handlers
    // (registered as they place) take precedence, and only a press
    // that claims no edge falls through to here.
    frame.handler().on_pointer_down(|app: &mut App, event| {
        event.button == Some(PointerButton::Primary) && app.model.selection.take().is_some()
    });
    // Mark-and-sweep by pass: entries the previous pass never used
    // are dropped here, everything else carries over — the steady
    // state is the visible text, shaped once.
    text_cache.sweep();
    let mut tcx = TextCtx {
        fonts: font_cx,
        layouts: layout_cx,
        scale: scale as f32,
        cache: text_cache,
    };
    let styles = display::Styles::new(scale);
    #[cfg(target_os = "linux")]
    let menu_hover = match hover.as_ref() {
        Some(Hovered::Menu(hover)) => Some(*hover),
        _ => None,
    };
    #[cfg(target_os = "linux")]
    let application_menu = menu::view(
        &mut tcx,
        menu::Description {
            state: menu,
            availability,
            raw: view.raw,
            graph: view.graph,
            hover: menu_hover,
            scale,
            width: viewport_width,
        },
        menu::Hooks {
            toggle: Rc::new(|app: &mut App, section| app.menu.toggle(section)),
            select: Rc::new(|app: &mut App, selection| app.choose_menu(selection)),
        },
    );
    #[cfg(not(target_os = "linux"))]
    let _ = (menu, availability);
    let content_viewport = content_viewport(viewport, scale);
    #[cfg(target_os = "linux")]
    layout::place(
        application_menu.bar,
        frame,
        Placement::new(
            Rect::new(0.0, 0.0, viewport_width, content_viewport.y0),
            Rect::new(0.0, 0.0, viewport_width, viewport_height),
        ),
    );
    let (tree_hover, graph_hover) = match hover.as_ref() {
        Some(Hovered::Tree(hovering)) => (Some(&hovering.hover), None),
        Some(Hovered::Graph(node)) => (None, Some(node)),
        #[cfg(target_os = "linux")]
        Some(Hovered::Menu(_)) => (None, None),
        None => (None, None),
    };
    // The Raw view is ONE bit, threaded as itself: name lookups
    // derive from it downstream, no policy swapped here, and the
    // model's configured policy rides along untouched.
    let sources = model.sources();
    let graph_node = model.graph_node();
    let margin = 12.0 * scale;
    // The width layout answers to: the window, less the graph panel
    // when it is up — the panel overlays the right side, and content
    // should break rather than run beneath it.
    let body_width = if view.graph {
        graph_panel(viewport, scale).x0 - 2.0 * margin
    } else {
        viewport_width - 2.0 * margin
    };
    let hover_node = graph_hover
        .and_then(|node| graph_view::node_value(&model.doc, node))
        .filter(|value| !matches!(value, Value::Record(_)));
    let body = raw::project(
        raw::ProjectDescription {
            sources,
            selection: model.tree_selection(),
            graph_node: graph_node.as_ref(),
            hover: tree_hover,
            hover_node: hover_node.as_ref(),
            collapse: &model.collapse,
            names: &model.names,
            raw: view.raw,
            styles: &styles,
            width: body_width,
            projection: projection::Projection::new(&model.foreign),
        },
        &mut tcx,
        raw::Hooks {
            // The selection transition: re-selecting the same path
            // keeps its editor state, and a reported text click seeds
            // or advances the editor's caret — focus and cursor
            // placement are one event.
            select: Rc::new(move |app: &mut App, path, click| {
                // A label pending has no path of its own — path()
                // names its PARENT — so a reported click is always a
                // real selection change (the pending row swallows its
                // own clicks before they can reach here).
                let fresh = match app.model.tree_selection() {
                    Some(selection::Selection::PendingEdge { .. }) | None => true,
                    Some(current) => current.path() != path,
                };
                if fresh {
                    app.model.selection = Some(Selected::Tree(selection::Selection::edge(
                        &app.model.sources(),
                        path,
                    )));
                } else if click.is_none()
                    && let Some(line) = app
                        .model
                        .tree_selection_mut()
                        .and_then(selection::Selection::edit_mut)
                {
                    // Re-selecting without a text click lands the
                    // caret at the end, same as a fresh mount.
                    line.cursor_to_end();
                }
                if let Some(click) = click
                    && let Some(line) = app
                        .model
                        .tree_selection_mut()
                        .and_then(selection::Selection::edit_mut)
                {
                    // A tap sequence never spans targets: the click
                    // that mounts an editor is its first, whatever
                    // the physical count says — selecting the cell
                    // was stage one, not half a double-click, and a
                    // quick click on a neighboring atom is not a
                    // double-click in this one.
                    let count = if fresh { 1 } else { click.count };
                    line.pointer_down(
                        &click.presentation,
                        &mut app.font_cx,
                        &mut app.layout_cx,
                        scale as f32,
                        LineEditPointerDown {
                            point: click.point,
                            shift: click.shift,
                            count,
                        },
                    );
                }
            }),
            toggle: Rc::new(|app: &mut App, path| {
                selection::toggle_collapse(
                    &sources::Sources {
                        doc: &app.model.doc,
                        library: &app.model.library,
                    },
                    &mut app.model.collapse,
                    &path,
                );
            }),
            rename: Rc::new(|app: &mut App, path, index| {
                if let Some(mut pending) = selection::pending_rename(&app.model.sources(), &path) {
                    // The index was hit-tested against the label that
                    // was clicked, in the label's own face; the seed
                    // shares its spelling, so the caret lands under
                    // the pointer in whatever face the editor draws.
                    if let Some(line) = pending.edit_mut() {
                        line.cursor_to(index);
                    }
                    app.model.selection = Some(Selected::Tree(pending));
                }
            }),
            edit: Rc::new(edit_ctx),
            pick: Rc::new(|app: &mut App, id| app.pick_identity(id)),
            insert: Rc::new(|app: &mut App, path| {
                if let Some(pending) = selection::pending_after(&app.model.sources(), &path) {
                    app.model.selection = Some(Selected::Tree(pending));
                }
            }),
        },
    );
    // The body rides Progred's scroll container: margins pad into the
    // content, the window is the viewport, and the app's clamped
    // offsets (ordinary model state) shift it. The horizontal
    // maximum answers to the LAYOUT width — content should only
    // scroll where even the block forms overflowed it — not the
    // window edge the viewport clips at.
    let content = layout::pad(vello::kurbo::Insets::uniform(margin), body);
    frame.max_scroll = ((content.extent.height() - content_viewport.height()) / scale).max(0.0);
    frame.max_scroll_x = ((content.extent.width - (body_width + 2.0 * margin)) / scale).max(0.0);
    let offset = Vec2::new(
        model.scroll_x.clamp(0.0, frame.max_scroll_x) * scale,
        model.scroll.clamp(0.0, frame.max_scroll) * scale,
    );
    let max_scroll = frame.max_scroll;
    let max_scroll_x = frame.max_scroll_x;
    let graph_panel_rect = view.graph.then(|| graph_panel(viewport, scale));
    layout::place_scrolled(
        content,
        frame,
        Placement::new(content_viewport, content_viewport),
        offset,
        move |app, update| {
            let point = Point::new(update.state.position.x, update.state.position.y);
            !graph_panel_rect.is_some_and(|panel| panel.contains(point))
                && app.scroll_document(
                    update,
                    scale,
                    content_viewport.height(),
                    max_scroll,
                    max_scroll_x,
                )
        },
    );
    // The graph pane draws over the document's right side; placed
    // after the body so its handlers win inside the panel.
    if view.graph {
        let panel = graph_panel(viewport, scale);
        let pane = graph_view::pane(
            &sources,
            &model.graph,
            model.graph_selection(),
            model.tree_selection(),
            graph_hover,
            tree_hover,
            &model.names,
            view.raw,
            &mut tcx,
            panel,
            &graph_view::Hooks {
                press_node: Rc::new(|app: &mut App, id, grab, world| {
                    // Grabbing a node drops a tree selection (its
                    // editor must not stay focused behind the drag);
                    // a graph selection stands until the release
                    // decides click or drag.
                    if matches!(app.model.selection, Some(Selected::Tree(_))) {
                        app.model.selection = None;
                    }
                    app.model.graph.press_node(id, grab, world);
                }),
                press_background: Rc::new(|app: &mut App, panel| {
                    app.model.graph.press_background(panel);
                }),
                drag_to: Rc::new(|app: &mut App, world, panel, px| {
                    app.model.graph.drag_to(world, panel, px)
                }),
                release: Rc::new(|app: &mut App| match app.model.graph.release() {
                    Some(graph_view::Release::ClickNode(id)) => {
                        app.model.selection =
                            Some(Selected::Graph(graph_view::GraphSelection::Node(id)));
                        true
                    }
                    Some(graph_view::Release::ClickBackground) => {
                        app.model.selection = None;
                        true
                    }
                    Some(graph_view::Release::Drag) => true,
                    None => false,
                }),
                scroll: Rc::new(|app: &mut App, delta, cursor, scale| {
                    app.model.graph.scroll(delta, cursor, scale);
                }),
                pick: Rc::new(|app: &mut App, id| app.pick_identity(id)),
            },
        );
        let rect = pane.extent.rect_at(Point::new(panel.x0, panel.y0));
        layout::place(pane, frame, Placement::new(rect, content_viewport));
    }

    // The pending row's popup draws after the body, so it overlays
    // and its click targets win.
    if let Some(popup) = frame.popup.take() {
        let hovered_entry = match tree_hover {
            Some(raw::Hover::Entry(index)) => Some(*index),
            _ => None,
        };
        let commit = |app: &mut App, action: &completion::EntryAction| match app.model.selection.take() {
            Some(Selected::Tree(selection::Selection::Pending { path, .. })) => {
                app.commit_value(path, action);
            }
            Some(Selected::Tree(selection::Selection::PendingEdge {
                parent, replacing, ..
            })) => {
                app.commit_label(parent, replacing, action);
            }
            selection => app.model.selection = selection,
        };
        let card = raw::popup_view(&mut tcx, &styles, &popup, hovered_entry, commit);
        // Below the anchor, unless it would run off the bottom and
        // fits above — then flip on top, as the TypeScript prototype
        // did. The card's extent is known before placement.
        let below = popup.anchor.y1 + 4.0 * scale;
        let above = popup.anchor.y0 - 4.0 * scale - card.extent.height();
        let y =
            if below + card.extent.height() > content_viewport.y1 && above >= content_viewport.y0 {
                above
            } else {
                below
            };
        let rect = card.extent.rect_at(Point::new(popup.anchor.x0, y));
        layout::place(card, frame, Placement::new(rect, content_viewport));
        frame.popup = Some(popup);
    }

    #[cfg(target_os = "linux")]
    if let Some((x, popup)) = application_menu.popup {
        let rect = popup.extent.rect_at(Point::new(x, content_viewport.y0));
        let headings = Rect::new(
            0.0,
            0.0,
            application_menu.heading_width,
            content_viewport.y0,
        );
        frame.handler().on_pointer_down(move |app, event| {
            let point = Point::new(event.state.position.x, event.state.position.y);
            event.button == Some(PointerButton::Primary)
                && !headings.contains(point)
                && !rect.contains(point)
                && app.menu.close()
        });
        frame.handler().on_pointer_down(move |_, event| {
            event.button == Some(PointerButton::Primary)
                && rect.contains(Point::new(event.state.position.x, event.state.position.y))
        });
        frame.handler().on_scroll(move |_, event| {
            rect.contains(Point::new(event.state.position.x, event.state.position.y))
        });
        layout::place(
            popup,
            frame,
            Placement::new(rect, Rect::new(0.0, 0.0, viewport_width, viewport_height)),
        );
    }
}

/// Dispatch-time access to the selection's editor. Retained-frame
/// dispatch can outlive the editor by a frame — deselect, then a move
/// in the same gesture — so absence declines rather than panics.
fn edit_ctx(app: &mut App) -> Option<EditCtx<'_>> {
    let App {
        model,
        font_cx,
        layout_cx,
        text_clipboard,
        ..
    } = app;
    let state = model
        .tree_selection_mut()
        .and_then(selection::Selection::edit_mut)?;
    Some(EditCtx {
        state,
        fonts: font_cx,
        layouts: layout_cx,
        clipboard: text_clipboard,
    })
}

#[cfg(test)]
mod frame_tests {
    use super::*;

    #[test]
    fn only_transitions_and_changed_frame_inputs_remint() {
        assert_eq!(frame_disposition(false, false), FrameDisposition::Retain);
        assert_eq!(
            frame_disposition(false, true),
            FrameDisposition::Remint {
                reveal_selection: false,
            }
        );
        assert_eq!(
            frame_disposition(true, false),
            FrameDisposition::Remint {
                reveal_selection: true,
            }
        );
        assert_eq!(
            frame_disposition(true, true),
            FrameDisposition::Remint {
                reveal_selection: true,
            }
        );
    }

    #[test]
    fn hover_resolution_keeps_only_real_hysteresis_state() {
        let hovering = raw::Hovering {
            hover: raw::Hover::Value(Vec::new()),
            rect: vello::kurbo::Rect::new(10.0, 10.0, 20.0, 20.0),
        };
        let current = Hovered::Tree(hovering.clone());
        assert_eq!(
            resolved_hover(
                Some(&current),
                None,
                Some(Point::new(24.0, 15.0)),
                false,
                8.0,
            ),
            Some(current.clone())
        );
        assert_eq!(
            resolved_hover(
                Some(&current),
                Some(HoverHit::Graph(Some(graph_view::GraphNode::Root))),
                Some(Point::ZERO),
                false,
                8.0,
            ),
            Some(Hovered::Graph(graph_view::GraphNode::Root))
        );
        assert_eq!(
            resolved_hover(
                Some(&current),
                Some(HoverHit::Tree(raw::HoverClaim::Direct(None))),
                Some(Point::ZERO),
                true,
                8.0,
            ),
            Some(current)
        );
        assert_eq!(resolved_hover(None, None, None, false, 8.0), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn menu_hover_uses_the_ordinary_settled_resolver() {
        let hover = menu::Hover::Item(menu::Selection::Save);
        assert_eq!(
            resolved_hover(
                None,
                Some(HoverHit::Menu(Some(hover))),
                Some(Point::new(20.0, 40.0)),
                false,
                8.0,
            ),
            Some(Hovered::Menu(hover))
        );
        assert_eq!(
            resolved_hover(
                Some(&Hovered::Menu(hover)),
                Some(HoverHit::Menu(None)),
                Some(Point::new(20.0, 40.0)),
                false,
                8.0,
            ),
            None
        );
    }
}
