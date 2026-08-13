//! Window shell: winit + Vello plumbing around pure frame drawing.
//! `run_frame` writes to any puri `Canvas`; here it streams into vello.

mod commands;
mod completion;
mod conventions;
mod display;
mod document;
mod filter;
mod frame;
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
mod navigate;
mod projection;
mod raw;
mod selection;
mod sources;
mod store;
#[cfg(test)]
mod test_values;

use crate::frame::{Dispatch, FrameDisposition, FrameVisibility, Hovered, frame_disposition};
use crate::model::{Model, Selected, ViewFlags};
use std::path::PathBuf;
use std::sync::Arc;

use parley::{FontContext, LayoutContext};
use puri::edit::TextClipboard;
use puri::handler::ImeEvent;
use ui_events::keyboard::KeyboardEvent;
use ui_events::pointer::PointerEvent;
use ui_events_winit::{WindowEventReducer, WindowEventTranslation};
use vello::kurbo::{Point, Rect, Size, Vec2};
use vello::peniko::{Brush, Color};
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::{self, CurrentSurfaceTexture};
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{Ime, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

/// Everything arriving through the event-loop proxy.
pub(crate) enum UserEvent {
    #[cfg(target_os = "macos")]
    MacMenu(macos_menu::Event),
    Menu(menu::Selection),
    Discard(bool),
}

/// The action a discard confirmation gates. One at a time: requests
/// while a sheet is up are dropped.
pub(crate) enum AfterDiscard {
    New,
    Open,
    Quit,
}

pub(crate) enum RenderState {
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
pub(crate) const CLIPBOARD_FORMAT: &str = "com.progred.value";

pub(crate) struct SystemTextClipboard;

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

pub(crate) struct App {
    pub(crate) context: RenderContext,
    pub(crate) renderers: Vec<Option<Renderer>>,
    pub(crate) state: RenderState,
    pub(crate) scene: Scene,
    pub(crate) font_cx: FontContext,
    pub(crate) layout_cx: LayoutContext<Brush>,
    pub(crate) text_clipboard: SystemTextClipboard,
    pub(crate) text_cache: puri::text::TextCache,
    pub(crate) model: Model,
    /// Where the document lives; `None` is untitled until the first
    /// save asks for a path.
    pub(crate) doc_path: Option<PathBuf>,
    /// The notation's file-local binder table, surviving load → save
    /// so spellings round-trip; never part of the model, invisible
    /// in the document.
    pub(crate) binders: gid::Binders,
    #[cfg(target_os = "macos")]
    pub(crate) native_menu: macos_menu::Menu,
    pub(crate) menu: menu::State,
    /// Last pointer position, for anchoring pinch zoom.
    pub(crate) cursor: Point,
    /// The pointer position while it is inside the window. It is an
    /// input to placement's internal hover resolution.
    pub(crate) pointer: Option<Point>,
    /// Derived from pointer input and settled geometry. Kept outside
    /// the model for air hysteresis, pressed-gesture freezing, and the
    /// event-to-redraw handoff.
    pub(crate) hover: Option<Hovered>,
    /// A button is down: gestures keep the hover they began with, so
    /// hover resolution stands down until release.
    pub(crate) pressed: bool,
    /// Whether settled geometry has resolved `hover` for the
    /// next draw. Geometry-changing redraw sources clear it.
    pub(crate) hover_is_current: bool,
    /// The selection identity last scrolled into view — path AND
    /// variant, since Enter keeps the path while opening a pending —
    /// so reveal fires once per change and never fights manual
    /// scrolling.
    pub(crate) revealed: Option<(document::Path, std::mem::Discriminant<selection::Selection>)>,
    pub(crate) dispatch: Option<Dispatch>,
    pub(crate) reducer: WindowEventReducer,
    /// Routes the discard sheet's answer back into the loop.
    pub(crate) proxy: winit::event_loop::EventLoopProxy<UserEvent>,
    pub(crate) pending_discard: Option<AfterDiscard>,
}

pub(crate) fn menu_height(scale: f64) -> f64 {
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

pub(crate) fn content_viewport(viewport: Size, scale: f64) -> Rect {
    Rect::new(
        0.0,
        menu_height(scale).min(viewport.height),
        viewport.width,
        viewport.height,
    )
}

pub(crate) fn graph_panel(viewport: Size, scale: f64) -> Rect {
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
pub(crate) fn edge_path(selection: &Option<Selected>) -> Option<document::Path> {
    match selection {
        Some(Selected::Tree(selection::Selection::Edge { path, .. })) => Some(path.clone()),
        _ => None,
    }
}

/// No modifiers at all — the gate for the bare editing keys.
pub(crate) fn plain(event: &KeyboardEvent) -> bool {
    !(event.modifiers.ctrl()
        || event.modifiers.meta()
        || event.modifiers.alt()
        || event.modifiers.shift())
}

pub(crate) fn dialog() -> rfd::FileDialog {
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
                            || match navigate::step_selection(
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

impl App {
    pub(crate) fn title(&self) -> String {
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

    pub(crate) fn refresh_title(&self) {
        if let RenderState::Active { window, .. } = &self.state {
            window.set_title(&self.title());
        }
    }

    /// Menu enablement follows the model: gray what can't act. Save
    /// stays live for untitled documents — it defers to the save
    /// panel, per platform convention.
    pub(crate) fn sync_menus(&self) {
        #[cfg(target_os = "macos")]
        self.native_menu
            .sync(self.menu_availability(), self.model.view);
    }

    pub(crate) fn menu_availability(&self) -> menu::Availability {
        menu::Availability {
            save: self.model.history.dirty() || self.doc_path.is_none(),
            undo: self.model.history.can_undo(),
            redo: self.model.history.can_redo(),
        }
    }

    pub(crate) fn handle_menu_selection(&mut self, event_loop: &ActiveEventLoop, selection: menu::Selection) {
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

    pub(crate) fn choose_menu(&mut self, selection: menu::Selection) {
        self.menu.close();
        let _ = self.proxy.send_event(UserEvent::Menu(selection));
    }

    pub(crate) fn menu_key(&mut self, event: &KeyboardEvent) -> bool {
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
    pub(crate) fn step_history(&mut self, back: bool) {
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
    pub(crate) fn request_discard(&mut self, event_loop: &ActiveEventLoop, then: AfterDiscard) {
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
    pub(crate) fn proceed(&mut self, event_loop: &ActiveEventLoop, then: AfterDiscard) {
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
    pub(crate) fn menu_save(&mut self, save_as: bool) {
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
    pub(crate) fn adopt_model(&mut self, doc: document::Document, path: Option<PathBuf>, binders: gid::Binders) {
        self.binders = binders;
        let view = self.model.view;
        self.model = Model {
            doc,
            selection: None,
            collapse: selection::Collapse::default(),
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

    pub(crate) fn adopt_doc_path(&mut self, path: PathBuf) {
        self.doc_path = Some(path);
        if let RenderState::Active { window, .. } = &self.state {
            window.set_title(&self.title());
            window.request_redraw();
        }
    }

    /// Scroll-to-reveal, computed from the freshly retained dispatch
    /// pass BEFORE anything draws, so the reveal lands in the next


    /// Renders the current model to the surface, from `RedrawRequested`.
    pub(crate) fn redraw(&mut self) {
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


