//! One window's editor and its per-window execution state.

use crate::clipboard::SystemTextClipboard;
use crate::command::{Command, DocCommand};
use crate::frame::FrameState;
use crate::input::{PendingGesture, PendingPointer, PendingScroll, continuous_input};
#[cfg(target_os = "macos")]
use crate::macos_window;
use crate::model::Model;
#[cfg(test)]
use crate::modifiers;
#[cfg(target_arch = "wasm32")]
use crate::web_worker;
use crate::{UserEvent, cursor_icon};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::{canonical, text_dialog, text_store};
use crate::{
    command, computations, gesture, gid_text, menu, navigate, selection, sources, stack, styles,
    timers, workspace,
};
use kurbo::{Point, Rect, Size};
use peniko::Brush;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use parley::{FontContext, LayoutContext};
#[cfg(not(target_arch = "wasm32"))]
use puri_vello::compositor::Resources;
use ui_events::keyboard::{KeyboardEvent, Modifiers};
use ui_events_winit::WindowEventReducer;
#[cfg(not(target_arch = "wasm32"))]
use vello::util::RenderSurface;
#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::window::{CursorIcon, Window, WindowId};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) enum RenderState {
    Active {
        surface: Box<RenderSurface<'static>>,
        valid_surface: bool,
        window: Arc<Window>,
    },
    Suspended(Option<Arc<Window>>),
}

#[cfg(target_arch = "wasm32")]
pub(crate) enum RenderState {
    Active {
        canvas: HtmlCanvasElement,
        window: Arc<Window>,
    },
    Suspended(Option<Arc<Window>>),
}

/// The action a discard confirmation gates. One at a time per window:
/// requests while its sheet is up are dropped.
pub(crate) enum AfterDiscard {
    /// Close this window; while quitting, the chain then advances.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    CloseWindow,
    Replace {
        doc: gid::Document,
        binders: gid_text::Binders,
    },
    #[cfg(target_arch = "wasm32")]
    Quit,
}

/// How finished Progred is, and the commit this build came from when it was
/// told: in the web editor's menu bar and window titles, so a screenshot
/// says which version it shows.
pub(crate) fn stage() -> String {
    match option_env!("PROGRED_COMMIT").filter(|commit| !commit.is_empty()) {
        Some(commit) => format!("pre-alpha {commit}"),
        None => "pre-alpha".to_owned(),
    }
}

/// One window editing one document: its own CellId universe, model,
/// history, interaction state, and measurement caches (the font
/// context is a cheap clone over shared font data). The dispatch
/// world type.
pub(crate) struct Editor {
    pub(crate) palette: styles::Palette,
    pub(crate) command_modifier: puri::keyboard::CommandModifier,
    pub(crate) computations: computations::Computations,
    pub(crate) timers: timers::Timers,
    pub(crate) focused: bool,
    /// This window draws its own menu bar (the drawn menu system).
    pub(crate) drawn_menu: bool,
    pub(crate) state: RenderState,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) paint_resources: Resources,
    pub(crate) font_cx: FontContext,
    pub(crate) layout_cx: LayoutContext<Brush>,
    pub(crate) text_clipboard: SystemTextClipboard,
    pub(crate) text_cache: puri::text::TextCache,
    pub(crate) stack: stack::Stack<Editor>,
    pub(crate) model: Model,
    /// Where the document lives; `None` is untitled until the first
    /// save asks for a path.
    pub(crate) doc_path: Option<PathBuf>,
    /// The text bridge's file-local binder table, surviving load → save
    /// so spellings round-trip; never part of the model, invisible
    /// in the document.
    pub(crate) text_binders: gid_text::Binders,
    pub(crate) menu: menu::State,
    /// Last observed pointer position, including outside-window drag motion.
    pub(crate) cursor: Point,
    /// The pointer position while it is inside the window. It is an
    /// input to placement's internal hover resolution.
    pub(crate) pointer: Option<Point>,
    /// Current platform modifier state, an ordinary frame input.
    pub(crate) modifiers: Modifiers,
    /// A button is down: gestures keep the hover they began with, so
    /// hover resolution stands down until release.
    pub(crate) pressed: bool,
    /// The continuation installed by the accepting projection handler.
    pub(crate) gesture: Option<gesture::Active<Editor>>,
    pub(crate) reducer: WindowEventReducer,
    /// Routes the discard sheet's answer back into the loop.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) proxy: Option<winit::event_loop::EventLoopProxy<UserEvent>>,
    pub(crate) pending_discard: Option<AfterDiscard>,
}

/// Per-window execution state. Widgets receive only `editor`, never this runner.
pub(crate) struct EditorRunner {
    pub(crate) editor: Editor,
    pub(crate) frame: FrameState,
    pub(crate) cursor_icon: CursorIcon,
    /// Consecutive scroll packets are a batch of observed samples.
    /// Hold them until paint or another event establishes an ordering
    /// boundary, then dispatch the batch through the retained frame.
    pub(crate) pending_scroll: Option<PendingScroll>,
    pub(crate) pending_gesture: Option<PendingGesture>,
    /// Pointer motion is continuous frame input, like scrolling.
    /// Keep the samples until paint or a
    /// discrete event establishes an ordering boundary. This is a
    /// platform-independent frame contract, but matters especially on
    /// macOS: Winit deliberately emits `CursorMoved` before every
    /// scroll to refresh the otherwise position-less `MouseWheel`
    /// event's implicit cursor state. Treating that synthetic refresh
    /// as a barrier made projection work keep AppKit from reaching its
    /// redraw boundary under continuous input.
    ///
    /// History: https://github.com/rust-windowing/winit/issues/942
    /// and https://github.com/rust-windowing/winit/pull/1490
    ///
    /// Handlers receive earlier samples in `PointerUpdate::coalesced`
    /// and the latest in `current`, including during a drag.
    pub(crate) pending_pointer: Option<PendingPointer>,
    /// The composition area last given to the window; `None` leaves IME off.
    input_area: Option<Rect>,
}

impl EditorRunner {
    pub(crate) fn new(editor: Editor) -> Self {
        Self {
            editor,
            frame: FrameState::default(),
            cursor_icon: CursorIcon::Default,
            pending_scroll: None,
            pending_gesture: None,
            pending_pointer: None,
            input_area: None,
        }
    }

    /// Flush before discrete input. A paint request already presents the
    /// resulting frame, so only other events need a future redraw requested.
    pub(crate) fn flush_before_window_event(&mut self, event: &WindowEvent) -> bool {
        !continuous_input(event)
            && self.flush_pending_continuous()
            && !matches!(event, WindowEvent::RedrawRequested)
    }

    /// Mirror the installed frame into the window state the platform owns:
    /// the pointer cursor, and whether and where text composition happens.
    pub(crate) fn sync_window(&mut self, window: &Window) {
        let next = cursor_icon(self.frame.hover.as_ref());
        if next != self.cursor_icon {
            window.set_cursor(next);
            self.cursor_icon = next;
        }
        let area = self.frame.input_area;
        if area != self.input_area {
            if area.is_some() != self.input_area.is_some() {
                window.set_ime_allowed(area.is_some());
            }
            if let Some(area) = area {
                window.set_ime_cursor_area(
                    PhysicalPosition::new(area.x0, area.y0),
                    PhysicalSize::new(area.width(), area.height()),
                );
            }
            self.input_area = area;
        }
    }

    pub(crate) fn adopt_model(
        &mut self,
        doc: gid::Document,
        path: Option<PathBuf>,
        text_binders: gid_text::Binders,
    ) {
        let Self {
            editor,
            frame,
            pending_scroll,
            pending_gesture,
            pending_pointer,
            cursor_icon: _,
            input_area: _,
        } = self;
        *frame = FrameState::default();
        *pending_scroll = None;
        *pending_gesture = None;
        *pending_pointer = None;
        editor.replace_document(doc, path, text_binders);
        if let RenderState::Active { window, .. } = &editor.state {
            let window = window.clone();
            let size = window.inner_size();
            self.refresh_frame(
                window.scale_factor(),
                Size::new(size.width as f64, size.height as f64),
            );
            self.sync_window(&window);
            window.request_redraw();
        }
    }
}

pub(crate) fn new_editor(
    palette: styles::Palette,
    command_modifier: puri::keyboard::CommandModifier,
    drawn_menu: bool,
    stack: stack::Stack<Editor>,
    font_cx: FontContext,
    doc: gid::Document,
    doc_path: Option<PathBuf>,
    text_binders: gid_text::Binders,
    proxy: Option<winit::event_loop::EventLoopProxy<UserEvent>>,
) -> Editor {
    #[cfg(target_arch = "wasm32")]
    let computations = proxy
        .as_ref()
        .map(|_| computations::Computations::new(web_worker::executor(), web_worker::wake))
        .unwrap_or_default();
    #[cfg(not(target_arch = "wasm32"))]
    let computations = proxy
        .clone()
        .map(|proxy| {
            let executor = incremental::background::Executor::threaded(std::num::NonZeroUsize::MIN)
                .expect("start computation worker");
            computations::Computations::new(executor, move || {
                let _ = proxy.send_event(UserEvent::ComputationFinished);
            })
        })
        .unwrap_or_default();
    Editor {
        palette,
        command_modifier,
        computations,
        timers: timers::Timers::default(),
        focused: false,
        drawn_menu,
        state: RenderState::Suspended(None),
        #[cfg(not(target_arch = "wasm32"))]
        paint_resources: Resources::default(),
        font_cx,
        layout_cx: LayoutContext::new(),
        text_cache: puri::text::TextCache::default(),
        #[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
        text_clipboard: SystemTextClipboard,
        #[cfg(any(test, target_arch = "wasm32"))]
        text_clipboard: SystemTextClipboard::default(),
        stack,
        model: Model::new(doc),
        doc_path,
        text_binders,
        menu: menu::State::default(),
        cursor: Point::ZERO,
        pointer: None,
        modifiers: Modifiers::empty(),
        pressed: false,
        gesture: None,
        reducer: WindowEventReducer::default(),
        proxy,
        pending_discard: None,
    }
}

impl Editor {
    pub(crate) fn window_id(&self) -> Option<WindowId> {
        self.window().map(|window| window.id())
    }

    pub(crate) fn window(&self) -> Option<Arc<Window>> {
        match &self.state {
            RenderState::Active { window, .. } => Some(window.clone()),
            RenderState::Suspended(window) => window.clone(),
        }
    }

    pub(crate) fn advance_gesture(&mut self, samples: &[Point]) -> bool {
        if let Some(mut gesture) = self.gesture.take() {
            let changed = gesture.advance(self, samples);
            self.gesture = Some(gesture);
            if changed {
                self.refresh_title();
            }
            true
        } else {
            false
        }
    }

    pub(crate) fn finish_gesture(&mut self) -> bool {
        if let Some(mut gesture) = self.gesture.take() {
            gesture.finish(self);
            true
        } else {
            false
        }
    }

    /// The current document read over the app's library.
    pub(crate) fn sources(&self) -> sources::Sources<'_> {
        sources::Sources {
            doc: &self.model.doc,
            libraries: &self.stack.libraries,
        }
    }

    pub(crate) fn title(&self) -> String {
        // The macOS convention: the display name — the dirty state is
        // the close button's dot, the location the proxy icon — then
        // the app's stage, as a beta's title says it is one.
        #[cfg(target_os = "macos")]
        return format!(
            "{} — Progred {}",
            match &self.doc_path {
                Some(path) => path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string()),
                None => "Untitled".to_string(),
            },
            stage()
        );
        #[cfg(not(target_os = "macos"))]
        {
            let dirty = if self.model.dirty() { " •" } else { "" };
            match &self.doc_path {
                Some(path) => format!("Progred {} — {}{dirty}", stage(), path.display()),
                None => format!("Progred {} — untitled{dirty}", stage()),
            }
        }
    }

    pub(crate) fn refresh_title(&self) {
        if let Some(window) = self.window() {
            window.set_title(&self.title());
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::WindowExtMacOS;
                window.set_document_edited(self.model.dirty());
                macos_window::set_represented(&window, self.doc_path.as_deref());
            }
        }
    }

    pub(crate) fn menu_toggles(&self) -> command::Toggles {
        let area = self.model.workspace.selected_or_document(
            self.model
                .selection
                .as_ref()
                .map(selection::Selection::root),
        );
        command::Toggles {
            raw: area.projection == workspace::Projection::Raw,
            debug_geometry: self.model.view.debug_geometry,
            hidden: area.hidden.clone(),
        }
    }

    pub(crate) fn menu_availability(&self) -> command::Availability {
        let selected_root = self
            .model
            .selection
            .as_ref()
            .map(selection::Selection::root);
        command::Availability {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            save: self.model.dirty() || self.doc_path.is_none(),
            undo: self.model.history.can_undo(),
            redo: self.model.history.can_redo(),
            open_pane: workspace::can_open(self.model.doc.root.as_ref())
                && self
                    .model
                    .selection
                    .as_ref()
                    .and_then(|selection| selection.value(&self.sources()))
                    .is_some(),
            move_up: selected_root
                .is_some_and(|root| self.model.workspace.can_move(root, workspace::Move::Up)),
            move_down: selected_root
                .is_some_and(|root| self.model.workspace.can_move(root, workspace::Move::Down)),
            move_left: selected_root
                .is_some_and(|root| self.model.workspace.can_move(root, workspace::Move::Left)),
            move_right: selected_root
                .is_some_and(|root| self.model.workspace.can_move(root, workspace::Move::Right)),
        }
    }

    fn open_selected_in_pane(&mut self, side: workspace::Side) -> bool {
        let next = self.model.selection.as_ref().and_then(|selection| {
            let value = selection.value(&self.sources())?.clone();
            workspace::append(self.model.doc.root.as_ref()?, side, value)
        });
        if let Some((value, path)) = next {
            let before = self.model.snapshot();
            Rc::make_mut(&mut self.model.doc).root = Some(value);
            self.model.history.record(before);
            self.model
                .workspace
                .sync_declared(&workspace::declarations(self.model.doc.root.as_ref()));
            let root = self
                .model
                .workspace
                .column(side)
                .panes
                .last()
                .unwrap()
                .view
                .root
                .clone();
            self.model.selection = Some(selection::Selection::edge(&root, path));
            self.refresh_title();
            true
        } else {
            false
        }
    }

    fn move_selected_pane(&mut self, direction: workspace::Move) -> bool {
        let next = self.model.selection.as_ref().and_then(|selection| {
            let workspace::Target::Pane { path } = selection.root().target() else {
                return None;
            };
            let suffix = selection.path().strip_prefix(path.as_slice())?.to_vec();
            let (value, path) =
                workspace::move_value(self.model.doc.root.as_ref()?, path, direction)?;
            Some((value, path, suffix, selection.root().clone()))
        });
        if let Some((value, path, suffix, root)) = next {
            let before = self.model.snapshot();
            Rc::make_mut(&mut self.model.doc).root = Some(value);
            self.model.history.record(before);
            let next_root = workspace::Root::pane(path.clone());
            if let Some(view) = self.model.workspace.view_mut(&root) {
                view.root = next_root.clone();
                view.annotations = Default::default();
            }
            if let Some(selection) = self.model.selection.as_mut() {
                selection.relocate(next_root, path.into_iter().chain(suffix).collect());
            }
            self.model
                .workspace
                .sync_declared(&workspace::declarations(self.model.doc.root.as_ref()));
            self.refresh_title();
            true
        } else {
            false
        }
    }

    pub(crate) fn choose_menu(&mut self, command: Command, geometry: navigate::Geometry<'_>) {
        self.menu.close();
        match command {
            Command::Doc(command) => self.run_doc_command(command, geometry),
            Command::App(_) => {
                if let Some(proxy) = &self.proxy {
                    let _ = proxy.send_event(UserEvent::Command(command));
                }
            }
        }
    }

    /// A document command against this editor, including the redraw
    /// its view changes require.
    pub(crate) fn run_doc_command(
        &mut self,
        command: DocCommand,
        geometry: navigate::Geometry<'_>,
    ) {
        match command {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            DocCommand::Save => self.menu_save(false),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            DocCommand::SaveAs => self.menu_save(true),
            DocCommand::Undo => self.step_history(true, geometry),
            DocCommand::Redo => self.step_history(false, geometry),
            DocCommand::OpenPaneLeft => {
                self.open_selected_in_pane(workspace::Side::Left);
            }
            DocCommand::OpenPaneRight => {
                self.open_selected_in_pane(workspace::Side::Right);
            }
            DocCommand::MovePaneUp
            | DocCommand::MovePaneDown
            | DocCommand::MovePaneLeft
            | DocCommand::MovePaneRight => {
                let direction = match command {
                    DocCommand::MovePaneUp => workspace::Move::Up,
                    DocCommand::MovePaneDown => workspace::Move::Down,
                    DocCommand::MovePaneLeft => workspace::Move::Left,
                    DocCommand::MovePaneRight => workspace::Move::Right,
                    _ => unreachable!(),
                };
                self.move_selected_pane(direction);
            }
            DocCommand::Raw => {
                let selected = self
                    .model
                    .selection
                    .as_ref()
                    .map(selection::Selection::root)
                    .cloned();
                self.model.workspace.toggle_projection(selected.as_ref());
            }
            DocCommand::Projection(library) => {
                let selected = self
                    .model
                    .selection
                    .as_ref()
                    .map(selection::Selection::root)
                    .cloned();
                self.model
                    .workspace
                    .toggle_library_projection(selected.as_ref(), library);
            }
            DocCommand::DebugGeometry => {
                self.model.view.debug_geometry = !self.model.view.debug_geometry
            }
        }
        // Execution only mutates; the caller owns frame scheduling —
        // the drawn dispatch through its disposition, the native path
        // in `run_command`.
    }

    pub(crate) fn menu_key(
        &mut self,
        event: &KeyboardEvent,
        geometry: navigate::Geometry<'_>,
    ) -> bool {
        let availability = self.menu_availability();
        if let Some(command) = menu::shortcut(event, self.command_modifier)
            .filter(|command| availability.enabled(*command))
        {
            self.choose_menu(command, geometry);
            return true;
        }
        match menu::navigate(
            &mut self.menu,
            &menu::definition(self.stack.projections().map(|(library, _)| library)),
            availability,
            event,
        ) {
            menu::Navigation::Activate(command) => {
                self.choose_menu(command, geometry);
                true
            }
            menu::Navigation::Handled => true,
            menu::Navigation::Pass => self.menu.captures_key(event),
        }
    }

    /// Undo or redo one step, restoring the snapshot's document and
    /// selection; the displaced state crosses to the other stack.
    pub(crate) fn step_history(&mut self, back: bool, geometry: navigate::Geometry<'_>) {
        self.finish_gesture();
        if self.model.step_history(back) {
            geometry.reveal_selection(self);
            self.refresh_title();
        }
    }

    /// Save saves in place, or asks for a path when untitled; save-as
    /// always asks. Write-through editing means the GID document is always
    /// current, so there is nothing to flush first. A cancelled dialog
    /// saves nothing.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) fn menu_save(&mut self, save_as: bool) {
        let in_place = (!save_as).then(|| self.doc_path.clone()).flatten();
        let target = in_place.or_else(|| {
            let file_name = self
                .doc_path
                .as_deref()
                .and_then(|path| path.file_name())
                .map_or_else(
                    || "untitled.gid".to_owned(),
                    |name| name.to_string_lossy().into_owned(),
                );
            text_dialog().set_file_name(file_name).save_file()
        });
        if let Some(path) = target {
            match text_store::save(&path, &self.model.doc, &self.text_binders) {
                Ok(()) => {
                    self.model.mark_saved();
                    self.finish_gesture();
                    self.adopt_doc_path(canonical(path));
                }
                Err(error) => {
                    eprintln!("failed to save {}: {error}", path.display());
                }
            }
        }
    }

    /// Replace document-owned state, retaining the window and its platform
    /// resources. In particular, the input reducer still knows the physical
    /// pointer position and held modifiers; old gestures do not survive.
    /// Libraries decide how everything is drawn and what cells resolve to,
    /// so a new stack keeps the document, selection, and folds but discards
    /// gestures and memoized work computed under the old one.
    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn replace_stack(&mut self, stack: stack::Stack<Editor>) {
        self.finish_gesture();
        self.computations.reset();
        self.stack = stack;
    }

    pub(crate) fn replace_document(
        &mut self,
        doc: gid::Document,
        path: Option<PathBuf>,
        text_binders: gid_text::Binders,
    ) {
        self.finish_gesture();
        // Exhaustive: a new Editor field must explicitly choose its lifetime here.
        let Self {
            palette: _,
            computations,
            command_modifier: _,
            timers,
            focused: _,
            drawn_menu: _,
            state: _,
            #[cfg(not(target_arch = "wasm32"))]
            paint_resources,
            font_cx: _,
            layout_cx: _,
            text_clipboard: _,
            text_cache: _,
            stack: _,
            model,
            doc_path,
            text_binders: binders,
            menu,
            cursor: _,
            pointer: _,
            modifiers: _,
            pressed,
            gesture: _,
            reducer: _,
            proxy: _,
            pending_discard,
        } = self;
        #[cfg(target_os = "macos")]
        let changed_path = *doc_path != path;
        *pending_discard = None;
        computations.reset();
        *timers = timers::Timers::default();
        *pressed = false;
        *menu = menu::State::default();
        *binders = text_binders;
        *doc_path = path;
        model.replace_document(doc);
        #[cfg(not(target_arch = "wasm32"))]
        {
            *paint_resources = Resources::default();
        }
        self.refresh_title();
        #[cfg(target_os = "macos")]
        if changed_path && let Some(window) = self.window() {
            match self.doc_path.as_deref() {
                Some(path) => macos_window::rename_document_frame(&window, path),
                None => macos_window::clear_document_frame(&window),
            }
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) fn adopt_doc_path(&mut self, path: PathBuf) {
        // An in-place save keeps its frame claim; only a new document
        // identity renames it.
        #[cfg(target_os = "macos")]
        if let RenderState::Active { window, .. } = &self.state
            && self.doc_path.as_deref() != Some(path.as_path())
        {
            macos_window::rename_document_frame(window, &path);
        }
        self.doc_path = Some(path);
        self.refresh_title();
        if let RenderState::Active { window, .. } = &self.state {
            window.request_redraw();
        }
    }
}

#[cfg(test)]
pub(crate) fn test_editor(doc: gid::Document) -> Editor {
    test_editor_with_stack(doc, stack::load())
}

#[cfg(test)]
pub(crate) fn test_editor_with_stack(doc: gid::Document, stack: stack::Stack<Editor>) -> Editor {
    let mut editor = new_editor(
        styles::Theme::Light.palette(),
        modifiers::native(),
        false,
        stack,
        FontContext::new(),
        doc,
        None,
        Default::default(),
        None,
    );
    editor.model.workspace.document.root = test_root();
    editor.focused = true;
    editor
}

#[cfg(test)]
pub(crate) fn test_root() -> workspace::Root {
    thread_local! { static ROOT: workspace::Root = workspace::Root::document(); }
    ROOT.with(Clone::clone)
}
