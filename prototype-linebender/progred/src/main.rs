//! Window shell: winit + Vello plumbing around pure frame drawing.
//! `run_frame` writes to any puri `Canvas`; here it streams into vello.

mod conventions;
mod filter;
mod sources;
mod graph_view;
mod history;
mod raw;
mod store;

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use parley::{FontContext, LayoutContext};
use progred_graph::Id;
use puri::draw::{Canvas, GlyphRun, Shape};
use puri::edit::{EditCtx, LineEditState};
use puri::handler::{Handler, HasHandler, ImeEvent};
use puri::layout::place_top_left;
use puri::text::TextCtx;
use puri_vello::VelloCanvas;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};
use ui_events::pointer::{PointerButton, PointerEvent};
use ui_events_winit::{WindowEventReducer, WindowEventTranslation};
use vello::kurbo::{Affine, Point, Size, Stroke, Vec2};
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

/// The last rendered frame's dispatch outputs, retained until the
/// next redraw replaces them: the handler events feed, plus what the
/// shell's key fallbacks interpret. The user reacts to what was
/// presented, so its geometry is the honest hit-test target — and the
/// event path runs no pass at all.
struct Dispatch {
    handler: Handler<App>,
    descends: Vec<raw::Descend>,
    max_scroll: f64,
    popup: Option<raw::Popup>,
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
    /// Menu items whose state the shell keeps current: enablement
    /// for save/undo/redo, muda-owned check state for the graph.
    menu_items: MenuItems,
    /// Last pointer position, for anchoring pinch zoom.
    cursor: Point,
    /// The selection identity last scrolled into view — path AND
    /// variant, since Enter keeps the path while opening a pending —
    /// so reveal fires once per change and never fights manual
    /// scrolling.
    revealed: Option<(raw::Path, std::mem::Discriminant<raw::Selection>)>,
    dispatch: Option<Dispatch>,
    reducer: WindowEventReducer,
}

struct MenuIds {
    new: MenuId,
    open: MenuId,
    save: MenuId,
    save_as: MenuId,
    quit: MenuId,
    undo: MenuId,
    redo: MenuId,
    graph: MenuId,
    raw: MenuId,
}

struct MenuItems {
    save: MenuItem,
    undo: MenuItem,
    redo: MenuItem,
    graph: CheckMenuItem,
    raw: CheckMenuItem,
}

/// The View menu's frame inputs: which panes and layers this frame
/// shows.
#[derive(Clone, Copy)]
struct ViewFlags {
    graph: bool,
    /// Convention layers off: names and the list projection stand
    /// down, showing the document as the pure graph it is.
    raw: bool,
}

/// The menu bar: file commands own their key equivalents, so the
/// platform routes Cmd+S and friends here rather than through key
/// dispatch. Attachment is macOS-only until another platform is run.
fn build_menu() -> (Menu, MenuIds, MenuItems) {
    let accel = if cfg!(target_os = "macos") {
        Modifiers::META
    } else {
        Modifiers::CONTROL
    };
    let new = MenuItem::new("New", true, Some(Accelerator::new(Some(accel), Code::KeyN)));
    let open = MenuItem::new("Open…", true, Some(Accelerator::new(Some(accel), Code::KeyO)));
    let save = MenuItem::new("Save", true, Some(Accelerator::new(Some(accel), Code::KeyS)));
    let save_as = MenuItem::new(
        "Save As…",
        true,
        Some(Accelerator::new(Some(accel | Modifiers::SHIFT), Code::KeyS)),
    );
    let quit = MenuItem::new("Quit Progred", true, Some(Accelerator::new(Some(accel), Code::KeyQ)));
    let undo = MenuItem::new("Undo", true, Some(Accelerator::new(Some(accel), Code::KeyZ)));
    let redo = MenuItem::new(
        "Redo",
        true,
        Some(Accelerator::new(Some(accel | Modifiers::SHIFT), Code::KeyZ)),
    );
    let graph = CheckMenuItem::new(
        "Graph",
        true,
        false,
        Some(Accelerator::new(Some(accel), Code::KeyG)),
    );
    let raw = CheckMenuItem::new(
        "Raw",
        true,
        false,
        Some(Accelerator::new(Some(accel), Code::KeyR)),
    );
    let menu = Menu::new();
    let ids = MenuIds {
        new: new.id().clone(),
        open: open.id().clone(),
        save: save.id().clone(),
        save_as: save_as.id().clone(),
        quit: quit.id().clone(),
        undo: undo.id().clone(),
        redo: redo.id().clone(),
        graph: graph.id().clone(),
        raw: raw.id().clone(),
    };
    menu.append_items(&[
        &Submenu::with_items(
            "Progred",
            true,
            &[
                &PredefinedMenuItem::about(None, None),
                &PredefinedMenuItem::separator(),
                &quit,
            ],
        )
        .expect("app menu"),
        &Submenu::with_items(
            "File",
            true,
            &[
                &new,
                &open,
                &PredefinedMenuItem::separator(),
                &save,
                &save_as,
            ],
        )
        .expect("file menu"),
        &Submenu::with_items("Edit", true, &[&undo, &redo]).expect("edit menu"),
        &Submenu::with_items("View", true, &[&raw, &graph]).expect("view menu"),
    ])
    .expect("menu bar");
    let items = MenuItems {
        save,
        undo,
        redo,
        graph,
        raw,
    };
    (menu, ids, items)
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
fn edge_path(selection: &Option<Selected>) -> Option<raw::Path> {
    match selection {
        Some(Selected::Tree(raw::Selection::Edge { path, .. })) => Some(path.clone()),
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
    rfd::FileDialog::new().add_filter("progred", &["progred"])
}

impl ApplicationHandler<MenuEvent> for App {
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: MenuEvent) {
        if *event.id() == self.menu_ids.new {
            self.menu_new();
        } else if *event.id() == self.menu_ids.open {
            self.menu_open();
        } else if *event.id() == self.menu_ids.save {
            self.menu_save(false);
        } else if *event.id() == self.menu_ids.save_as {
            self.menu_save(true);
        } else if *event.id() == self.menu_ids.quit {
            if self.confirm_discard() {
                _event_loop.exit();
            }
        } else if *event.id() == self.menu_ids.undo {
            self.step_history(true);
        } else if *event.id() == self.menu_ids.redo {
            self.step_history(false);
        } else if (*event.id() == self.menu_ids.graph || *event.id() == self.menu_ids.raw)
            && let RenderState::Active { window, .. } = &self.state
        {
            window.request_redraw();
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

        // Pinch zooms the graph toward the cursor; winit delivers it
        // outside the pointer stream the reducer covers.
        if let WindowEvent::PinchGesture { delta, .. } = &event
            && self.menu_items.graph.is_checked()
        {
            let size = window.inner_size();
            let panel = graph_view::panel(size.width as f64, size.height as f64);
            let anchor = if panel.contains(self.cursor) {
                self.cursor - panel.center()
            } else {
                Vec2::ZERO
            };
            self.model.graph.zoom_at(1.0 + delta, anchor, scale);
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
            // the successor is minted immediately below; unhandled
            // events leave it standing. Until the first redraw there
            // is nothing to dispatch into.
            if (ime.is_some() || translation.is_some())
                && let Some(dispatch) = self.dispatch.take()
            {
                let size = window.inner_size();
                let viewport = size.height as f64;
                let handled = match (ime, translation) {
                    (Some(ime), _) => dispatch.handler.dispatch_ime(self, &ime),
                    // Keys nothing claims fall through to the collapse
                    // toggle and then selection stepping, so the
                    // selected string's editor always wins over both.
                    (None, Some(WindowEventTranslation::Keyboard(key_event))) => {
                        dispatch.handler.dispatch_key(self, &key_event)
                            || self.graph_key(&key_event)
                            || self.delete_key(&dispatch.descends, &key_event)
                            || self.insert_key(&dispatch.descends, &dispatch.popup, &key_event)
                            || self.collapse_key(&key_event)
                            || match raw::step_selection(
                                &dispatch.descends,
                                self.model.tree_selection(),
                                &key_event,
                            ) {
                                Some(path) => {
                                    self.model.selection = Some(Selected::Tree(
                                        raw::Selection::edge(&self.model.sources(), path),
                                    ));
                                    true
                                }
                                None => false,
                            }
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Down(button)))) => {
                        dispatch.handler.dispatch_pointer_down(self, &button)
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Move(update)))) => {
                        dispatch.handler.dispatch_pointer_move(self, &update)
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Up(button)))) => {
                        dispatch.handler.dispatch_pointer_up(self, &button)
                    }
                    // Scrolls nothing claims pan or zoom the graph
                    // under the cursor, else move the document.
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Scroll(update)))) => {
                        dispatch.handler.dispatch_scroll(self, &update)
                            || self.graph_scroll(&update, scale, size.width as f64, viewport)
                            || self.scroll_document(&update, scale, viewport, dispatch.max_scroll)
                    }
                    _ => false,
                };
                if handled {
                    let model = &mut self.model;
                    if let Some(Selected::Tree(selection)) = &mut model.selection {
                        let before = model.doc.clone();
                        // True on the first write of the editor's
                        // life: the run's one step opens here.
                        if raw::write_through(&mut model.doc, &model.library, selection) {
                            let path = selection.path().to_vec();
                            model.history.record(before, Some(path));
                            self.refresh_title();
                        }
                    }
                    // Mint the successor handler from the mutated
                    // state now, so the next event — even within the
                    // same gesture — never sees the spent one. The
                    // redraw derives the pixels from the same state.
                    self.retain_dispatch(scale, Size::new(size.width as f64, viewport));
                    self.reveal_selection(scale, viewport);
                    window.request_redraw();
                } else {
                    self.dispatch = Some(dispatch);
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                if self.confirm_discard() {
                    event_loop.exit();
                }
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
    let (menu, menu_ids, menu_items) = build_menu();
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
            names: conventions::Names::default(),
            library: conventions::library(),
            graph: graph_view::GraphView::default(),
            history: history::History::default(),
            scroll: 0.0,
        },
        doc_path,
        menu,
        menu_ids,
        menu_items,
        cursor: Point::ZERO,
        revealed: None,
        dispatch: None,
        reducer: WindowEventReducer::default(),
    };

    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");
}

/// The app's one selection: the tree's edge or pending, or the
/// graph's node or edge. A single slot, so selecting in either pane
/// inherently clears the other — there is nothing to synchronize.
// One instance lives in the model; the variants' size gap is moot.
#[allow(clippy::large_enum_variant)]
enum Selected {
    Tree(raw::Selection),
    Graph(graph_view::GraphSelection),
}

struct Model {
    doc: raw::Document,
    selection: Option<Selected>,
    collapse: raw::Collapse,
    /// The name policy: an editor setting, not document state, so it
    /// survives document swaps.
    names: conventions::Names,
    /// The built-in library, read under every document; never
    /// written, never saved.
    library: progred_graph::MutGid,
    graph: graph_view::GraphView,
    history: history::History,
    /// Vertical document scroll offset in logical pixels, so the
    /// position survives moving between monitor scales. May exceed
    /// the current maximum after a resize: placement clamps
    /// effectively, so a transient shrink-and-grow restores the
    /// position; scrolling collapses it to the clamped reality.
    scroll: f64,
}

impl Model {
    /// The reading context: this document over the editor's library.
    fn sources(&self) -> sources::Sources<'_> {
        sources::Sources {
            doc: &self.doc,
            library: &self.library,
        }
    }

    fn tree_selection(&self) -> Option<&raw::Selection> {
        match &self.selection {
            Some(Selected::Tree(selection)) => Some(selection),
            _ => None,
        }
    }

    fn tree_selection_mut(&mut self) -> Option<&mut raw::Selection> {
        match &mut self.selection {
            Some(Selected::Tree(selection)) => Some(selection),
            _ => None,
        }
    }

    fn graph_selection(&self) -> Option<&graph_view::GraphSelection> {
        match &self.selection {
            Some(Selected::Graph(selection)) => Some(selection),
            _ => None,
        }
    }

    /// The graph-selected node, for the tree's secondary marks.
    fn graph_node(&self) -> Option<&Id> {
        match self.graph_selection() {
            Some(graph_view::GraphSelection::Node(id)) => Some(id),
            _ => None,
        }
    }
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
    /// The pending row's completion popup, emitted during placement;
    /// drawn after the body and committed from at dispatch.
    popup: Option<raw::Popup>,
}

impl raw::HasPopup for Frame<'_> {
    fn popup(&mut self) -> &mut Option<raw::Popup> {
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
        let dirty = if self.model.history.dirty() { " •" } else { "" };
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
        self.menu_items
            .save
            .set_enabled(self.model.history.dirty() || self.doc_path.is_none());
        self.menu_items
            .undo
            .set_enabled(self.model.history.can_undo());
        self.menu_items
            .redo
            .set_enabled(self.model.history.can_redo());
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
                .map(|path| Selected::Tree(raw::Selection::edge(&self.model.sources(), path)));
            self.refresh_title();
            if let RenderState::Active { window, .. } = &self.state {
                let window = window.clone();
                let size = window.inner_size();
                let scale = window.scale_factor();
                self.retain_dispatch(
                    scale,
                    Size::new(size.width as f64, size.height as f64),
                );
                self.reveal_selection(scale, size.height as f64);
                window.request_redraw();
            }
        }
    }

    /// Unsaved changes gate for the document-replacing commands.
    fn confirm_discard(&self) -> bool {
        !self.model.history.dirty()
            || rfd::MessageDialog::new()
                .set_title("Unsaved Changes")
                .set_description("Discard unsaved changes?")
                .set_buttons(rfd::MessageButtons::OkCancel)
                .show()
                == rfd::MessageDialogResult::Ok
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
                Ok(()) => {
                    self.model.history.mark_saved();
                    // A run must not straddle the save mark, or edits
                    // after it would coalesce into a pre-save step.
                    raw::break_edit_run(self.model.tree_selection_mut());
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
    fn adopt_model(&mut self, doc: raw::Document, path: Option<PathBuf>) {
        self.model = Model {
            doc,
            selection: None,
            collapse: raw::Collapse::default(),
            names: self.model.names.clone(),
            library: conventions::library(),
            graph: graph_view::GraphView::default(),
            history: history::History::default(),
            scroll: 0.0,
        };
        self.doc_path = path;
        self.revealed = None;
        if let RenderState::Active { window, .. } = &self.state {
            let window = window.clone();
            window.set_title(&self.title());
            let size = window.inner_size();
            self.retain_dispatch(
                window.scale_factor(),
                Size::new(size.width as f64, size.height as f64),
            );
            window.request_redraw();
        }
    }

    /// A fresh untitled document, gated on unsaved changes.
    fn menu_new(&mut self) {
        if self.confirm_discard() {
            self.adopt_model(
                raw::Document {
                    root: None,
                    gid: progred_graph::MutGid::new(),
                },
                None,
            );
        }
    }

    fn menu_open(&mut self) {
        if !self.confirm_discard() {
            return;
        }
        if let Some(path) = dialog().pick_file() {
            match store::load(&path) {
                Ok(doc) => self.adopt_model(doc, Some(path)),
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

    /// Scroll-to-reveal, computed from the freshly retained dispatch
    /// pass BEFORE anything draws, so the reveal lands in the next
    /// presented frame with no corrective flash. Fires once per
    /// selection-identity change (path AND variant — Enter keeps the
    /// path while opening a pending), so it never fights manual
    /// scrolling. The target is the popup anchor while pending — it
    /// marks the authoring row — else the selection's rect.
    fn reveal_selection(&mut self, scale: f64, viewport: f64) {
        let reveal = self
            .model
            .tree_selection()
            .map(|s| (s.path().to_vec(), std::mem::discriminant(s)));
        if reveal == self.revealed {
            return;
        }
        self.revealed = reveal.clone();
        let Some(dispatch) = &self.dispatch else {
            return;
        };
        let target = dispatch.popup.as_ref().map(|popup| popup.anchor).or_else(|| {
            reveal.as_ref().and_then(|(path, _)| {
                dispatch
                    .descends
                    .iter()
                    .find(|descend| &descend.path == path)
                    .map(|descend| descend.rect)
            })
        });
        if let Some(rect) = target {
            let pad = 12.0 * scale;
            let mut scroll = self.model.scroll;
            // The pad is the landing margin, not the trigger: fully
            // visible rects are left alone, so a click near an edge
            // doesn't nudge.
            if rect.y1 > viewport {
                scroll += (rect.y1 + pad - viewport) / scale;
            }
            // Checked against the adjusted position, so when the rect
            // is taller than the viewport the top wins.
            let top = rect.y0 - (scroll - self.model.scroll) * scale;
            if top < 0.0 {
                scroll += (top - pad) / scale;
            }
            self.model.scroll = scroll.clamp(0.0, dispatch.max_scroll);
        }
    }

    fn view_flags(&self) -> ViewFlags {
        ViewFlags {
            graph: self.menu_items.graph.is_checked(),
            raw: self.menu_items.raw.is_checked(),
        }
    }

    /// Runs the pure pass for the current state and retains its
    /// dispatch outputs; no scene — pixels are the redraw's job.
    fn retain_dispatch(&mut self, scale: f64, viewport: Size) {
        let mut frame = Frame {
            scene: None,
            handler: Handler::new(),
            descends: Vec::new(),
            max_scroll: 0.0,
            popup: None,
        };
        let view = self.view_flags();
        run_frame(
            &mut frame,
            &self.model,
            view,
            &mut self.font_cx,
            &mut self.layout_cx,
            scale,
            viewport,
        );
        let Frame {
            handler,
            descends,
            max_scroll,
            popup,
            ..
        } = frame;
        self.dispatch = Some(Dispatch {
            handler,
            descends,
            max_scroll,
            popup,
        });
    }

    /// Scrolls over the graph panel drive its viewport — trackpad
    /// pixels pan, wheel lines zoom toward the cursor — instead of
    /// the document.
    fn graph_scroll(
        &mut self,
        update: &ui_events::pointer::PointerScrollEvent,
        scale: f64,
        width: f64,
        height: f64,
    ) -> bool {
        if !self.menu_items.graph.is_checked() {
            return false;
        }
        let panel = graph_view::panel(width, height);
        let position = Point::new(update.state.position.x, update.state.position.y);
        panel.contains(position) && {
            self.model
                .graph
                .scroll(&update.delta, position - panel.center(), scale);
            true
        }
    }

    /// Graph-view keys: Delete removes the graph selection — one
    /// detachment for an edge, full detachment everywhere for a node.
    /// The one selection slot means this and the document delete
    /// below can never both match; Escape falls through to the
    /// universal clear in `insert_key`.
    fn graph_key(&mut self, event: &KeyboardEvent) -> bool {
        event.state.is_down()
            && plain(event)
            && matches!(
                &event.key,
                Key::Named(NamedKey::Backspace | NamedKey::Delete)
            )
            && match self.model.graph_selection() {
                Some(selection) => {
                    let selection = selection.clone();
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
            && match &self.model.selection {
                // Only a real edge deletes; a pending's Backspace is
                // its cancel, handled by insert_key.
                Some(Selected::Tree(raw::Selection::Edge { path, recorded, .. })) => {
                    let path = path.clone();
                    // Backspacing through the value and once more to
                    // delete the edge is one gesture: when this edge
                    // has the open run, its frame (pre-run document,
                    // edge intact) already covers the deletion.
                    let covered = *recorded;
                    let before = self.model.doc.clone();
                    raw::delete_edge(&mut self.model.doc, &self.model.library, &path) && {
                        if !covered {
                            self.model.history.record(before, Some(path.clone()));
                            self.refresh_title();
                        }
                        let next = raw::selection_after_delete(descends, &path);
                        self.model.selection =
                            Some(Selected::Tree(raw::Selection::edge(&self.model.sources(), next)));
                        true
                    }
                }
                _ => false,
            }
    }

    /// The chosen entry's action — from the frame's popup, else the
    /// query's inferred atom.
    fn chosen_action(
        popup: &Option<raw::Popup>,
        query: &LineEditState,
        choice: usize,
    ) -> raw::EntryAction {
        popup
            .as_ref()
            .and_then(|p| p.entries.get(choice.min(p.entries.len().saturating_sub(1))))
            .map(|entry| entry.action.clone())
            .unwrap_or_else(|| {
                raw::EntryAction::Value(raw::resolve_query(query.text()))
            })
    }

    /// Commits a pointed-at identity into the open pending — the
    /// command-click gesture. A value-stage pending commits and
    /// selects the edge; a label stage advances to its value stage.
    /// False when nothing is pending, so the click falls through.
    fn pick_identity(&mut self, id: Id) -> bool {
        match self.model.selection.take() {
            Some(Selected::Tree(raw::Selection::Pending { path, .. })) => {
                self.commit_value(path, &raw::EntryAction::Value(id));
                true
            }
            Some(Selected::Tree(raw::Selection::PendingEdge { parent, .. })) => {
                self.commit_label(parent, &raw::EntryAction::Value(id));
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
    fn commit_value(&mut self, path: raw::Path, action: &raw::EntryAction) {
        let before = self.model.doc.clone();
        if raw::commit_pending(&mut self.model.doc, &self.model.library, &path, action) {
            self.model.history.record(before, None);
            self.refresh_title();
        }
        self.model.selection = Some(Selected::Tree(raw::Selection::edge(&self.model.sources(), path)));
    }

    /// A resolved label advances the pending edge to its value stage —
    /// or selects the existing edge when the label is taken. Minting a
    /// new-node label writes its name: an undo step.
    fn commit_label(&mut self, parent: raw::Path, action: &raw::EntryAction) {
        let before = self.model.doc.clone();
        let label = raw::resolve_entry(&mut self.model.doc, action);
        // Only a NAMED mint writes an edge; an unnamed one is just a
        // fresh id, no mutation to record.
        if matches!(action, raw::EntryAction::NewNode { name: Some(_) }) {
            self.model.history.record(before, None);
            self.refresh_title();
        }
        let mut path = parent;
        path.push(label);
        self.model.selection = Some(Selected::Tree(
            match self.model.sources().resolve(&path) {
                Some(_) => raw::Selection::edge(&self.model.sources(), path),
                None => raw::pending_value(path),
            },
        ));
    }

    /// Enter advances a pending stage or begins one (the chains live
    /// in raw). Plain Enter is a new peer BESIDE the selection: a
    /// sibling element in a list (Shift+Enter before), a new field on
    /// the parent record otherwise; the root has nothing beside it
    /// and takes the field on itself. The command chord authors
    /// WITHIN the selection: a field edge on the selected node — or,
    /// with Shift, a first element at the front, which is how lists
    /// begin. Labels author first, then values; list elements are
    /// one-stage value pendings, the projection minting the position.
    /// On an empty document Enter begins the root value. Escape
    /// clears the selection from anywhere, discarding any pending
    /// with the graph untouched; Backspace on an empty query cancels
    /// a pending back to its anchor instead, keeping the keyboard
    /// flow.
    fn insert_key(
        &mut self,
        descends: &[raw::Descend],
        popup: &Option<raw::Popup>,
        event: &KeyboardEvent,
    ) -> bool {
        event.state.is_down()
            && match &event.key {
                // While pending, vertical arrows drive the popup choice.
                Key::Named(direction @ (NamedKey::ArrowUp | NamedKey::ArrowDown)) => {
                    match &mut self.model.selection {
                        Some(Selected::Tree(
                            raw::Selection::Pending { choice, .. }
                            | raw::Selection::PendingEdge { choice, .. },
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
                    Some(Selected::Tree(raw::Selection::Pending {
                        path,
                        query,
                        choice,
                    })) => {
                        let action = Self::chosen_action(popup, &query, choice);
                        self.commit_value(path, &action);
                        true
                    }
                    Some(Selected::Tree(raw::Selection::PendingEdge {
                        parent,
                        query,
                        choice,
                    })) => {
                        let action = Self::chosen_action(popup, &query, choice);
                        self.commit_label(parent, &action);
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
                                raw::pending_insert(&sources, current.path(), shift)
                            }
                            Some(current) => {
                                raw::pending_enter(&sources, current.path(), shift)
                            }
                            None => raw::pending_root(&sources),
                        };
                        let began = started.is_some();
                        self.model.selection = started.map(Selected::Tree).or(selection);
                        began
                    }
                },
                Key::Named(NamedKey::Escape) => {
                    self.model.selection.take().is_some()
                }
                Key::Named(NamedKey::Backspace) => {
                    match &self.model.selection {
                        Some(Selected::Tree(raw::Selection::Pending { path, .. })) => {
                            let back = raw::selection_after_delete(descends, path);
                            // Cancelling the empty document's root
                            // pending deselects — reselecting it
                            // would pend again.
                            self.model.selection = (!(back.is_empty()
                                && self.model.doc.root.is_none()))
                            .then(|| {
                                Selected::Tree(raw::Selection::edge(&self.model.sources(), back))
                            });
                            true
                        }
                        Some(Selected::Tree(raw::Selection::PendingEdge { parent, .. })) => {
                            self.model.selection = Some(Selected::Tree(raw::Selection::edge(
                                &self.model.sources(),
                                parent.clone(),
                            )));
                            true
                        }
                        _ => false,
                    }
                }
                _ => false,
            }
    }

    /// Space toggles the selected node's collapse override; a focused
    /// string editor claims the key first and types a space instead.
    fn collapse_key(&mut self, event: &KeyboardEvent) -> bool {
        event.state.is_down()
            && matches!(&event.key, Key::Character(c) if c.as_str() == " ")
            && match self.model.tree_selection() {
                Some(selection) => {
                    let path = selection.path().to_vec();
                    raw::toggle_collapse(
                        &sources::Sources {
                            doc: &self.model.doc,
                            library: &self.model.library,
                        },
                        &mut self.model.collapse,
                        &path,
                    )
                }
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

        // Advance the force simulation while the graph is open; the
        // continuous redraw request below keeps it animating.
        self.sync_menus();
        let view = self.view_flags();
        if view.graph {
            self.model.graph.step(&self.model.doc);
        }
        self.scene.reset();
        let mut frame = Frame {
            scene: Some(&mut self.scene),
            handler: Handler::new(),
            descends: Vec::new(),
            max_scroll: 0.0,
            popup: None,
        };
        run_frame(
            &mut frame,
            &self.model,
            view,
            &mut self.font_cx,
            &mut self.layout_cx,
            scale,
            Size::new(width as f64, height as f64),
        );
        let Frame {
            handler,
            descends,
            max_scroll,
            popup,
            ..
        } = frame;
        self.dispatch = Some(Dispatch {
            handler,
            descends,
            max_scroll,
            popup,
        });

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

        if view.graph && self.model.graph.hot() {
            window.request_redraw();
        }
    }
}

fn run_frame(
    frame: &mut Frame<'_>,
    model: &Model,
    view: ViewFlags,
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<Brush>,
    scale: f64,
    viewport: Size,
) {
    let (viewport_width, viewport_height) = (viewport.width, viewport.height);
    // Empty space deselects — the one slot, whichever pane filled it.
    // Registered before the content places, so the descend handlers
    // (registered as they place) take precedence, and only a press
    // that claims no edge falls through to here.
    frame.handler().on_pointer_down(|app: &mut App, event| {
        event.button == Some(PointerButton::Primary) && app.model.selection.take().is_some()
    });

    let mut tcx = TextCtx {
        fonts: font_cx,
        layouts: layout_cx,
        scale: scale as f32,
    };
    let styles = raw::RawStyles::new(scale);
    // The Raw view stands the convention layers down for this frame:
    // names answer None and the list projection is bypassed. The
    // model's configured policy is untouched underneath.
    let none = conventions::Names::none();
    let names = if view.raw { &none } else { &model.names };
    let sources = model.sources();
    let body = raw::project(
        &sources,
        model.tree_selection(),
        model.graph_node(),
        &model.collapse,
        names,
        view.raw,
        &mut tcx,
        &styles,
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
                    Some(raw::Selection::PendingEdge { .. }) | None => true,
                    Some(current) => current.path() != path,
                };
                if fresh {
                    app.model.selection =
                        Some(Selected::Tree(raw::Selection::edge(&app.model.sources(), path)));
                } else if click.is_none()
                    && let Some(line) =
                        app.model.tree_selection_mut().and_then(raw::Selection::edit_mut)
                {
                    // Re-selecting without a text click lands the
                    // caret at the end, same as a fresh mount.
                    line.cursor_to_end();
                }
                if let Some(click) = click
                    && let Some(line) =
                        app.model.tree_selection_mut().and_then(raw::Selection::edit_mut)
                {
                    // A tap sequence never spans targets: the click
                    // that mounts an editor is its first, whatever
                    // the physical count says — selecting the node
                    // was stage one, not half a double-click, and a
                    // quick click on a neighboring atom is not a
                    // double-click in this one.
                    let count = if fresh { 1 } else { click.count };
                    line.pointer_down(
                        &mut app.font_cx,
                        &mut app.layout_cx,
                        scale as f32,
                        click.point,
                        click.shift,
                        count,
                    );
                }
            }),
            toggle: Rc::new(|app: &mut App, path| {
                raw::toggle_collapse(
                    &sources::Sources {
                        doc: &app.model.doc,
                        library: &app.model.library,
                    },
                    &mut app.model.collapse,
                    &path,
                );
            }),
            edit: Rc::new(edit_ctx),
            pick: Rc::new(|app: &mut App, id| app.pick_identity(id)),
        },
    );
    let margin = 12.0 * scale;
    frame.max_scroll = ((body.extent.height() + 2.0 * margin - viewport_height) / scale).max(0.0);
    place_top_left(
        body,
        frame,
        Point::new(
            margin,
            margin - model.scroll.clamp(0.0, frame.max_scroll) * scale,
        ),
    );
    // The graph pane draws over the document's right side; placed
    // after the body so its handlers win inside the panel.
    if view.graph {
        let panel = graph_view::panel(viewport_width, viewport_height);
        let pane = graph_view::pane(
            &sources,
            &model.graph,
            model.graph_selection(),
            model.tree_selection(),
            names,
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
                press_edge: Rc::new(|app: &mut App, source, label| {
                    app.model.selection = Some(Selected::Graph(
                        graph_view::GraphSelection::Edge { source, label },
                    ));
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
                pick: Rc::new(|app: &mut App, id| app.pick_identity(id)),
            },
        );
        place_top_left(pane, frame, Point::new(panel.x0, panel.y0));
    }

    // The pending row's popup draws after the body, so it overlays
    // and its click targets win.
    if let Some(popup) = frame.popup.take() {
        let card = raw::popup_view(&mut tcx, &styles, &popup, |app: &mut App, action| {
            match app.model.selection.take() {
                Some(Selected::Tree(raw::Selection::Pending { path, .. })) => {
                    app.commit_value(path, action);
                }
                Some(Selected::Tree(raw::Selection::PendingEdge { parent, .. })) => {
                    app.commit_label(parent, action);
                }
                selection => app.model.selection = selection,
            }
        });
        // Below the anchor, unless it would run off the bottom and
        // fits above — then flip on top, as the TypeScript prototype
        // did. The card's extent is known before placement.
        let below = popup.anchor.y1 + 4.0 * scale;
        let above = popup.anchor.y0 - 4.0 * scale - card.extent.height();
        let y = if below + card.extent.height() > viewport_height && above >= 0.0 {
            above
        } else {
            below
        };
        place_top_left(card, frame, Point::new(popup.anchor.x0, y));
        frame.popup = Some(popup);
    }
}

/// Dispatch-time access to the selection's editor. Retained-frame
/// dispatch can outlive the editor by a frame — deselect, then a move
/// in the same gesture — so absence declines rather than panics.
fn edit_ctx(app: &mut App) -> Option<EditCtx<'_>> {
    let state = app
        .model
        .tree_selection_mut()
        .and_then(raw::Selection::edit_mut)?;
    Some(EditCtx {
        state,
        fonts: &mut app.font_cx,
        layouts: &mut app.layout_cx,
    })
}
