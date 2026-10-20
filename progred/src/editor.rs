//! One window's editor and its per-window execution state.

use crate::UserEvent;
use crate::clipboard::Clipboard;
use crate::command::{Command, DocCommand};
use crate::display::widget::frame::Cursor;
use crate::display::widget::view::{Root, Target};
use crate::frame::FrameState;
use crate::input::{PendingGesture, PendingPointer, PendingScroll, continuous_input};
#[cfg(target_os = "macos")]
use crate::macos_window;
use crate::model::Model;
#[cfg(all(target_arch = "wasm32", target_feature = "atomics"))]
use crate::web_worker;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::{canonical, text_dialog, text_store};
use crate::{
    command, computations,
    display::widget::{navigation::Geometry, style},
    gesture, gid_text, menu, selection, sources, stack, timers, workspace,
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
    /// Offer the browser's file picker; its file arrives as an event.
    #[cfg(target_arch = "wasm32")]
    Open,
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

/// What `window` draws into, in physical pixels.
pub(crate) fn viewport(window: &Window) -> Size {
    let size = window.inner_size();
    Size::new(size.width.into(), size.height.into())
}

/// One window editing one document: its own CellId universe, model,
/// history, interaction state, and measurement caches (the font
/// context is a cheap clone over shared font data). The dispatch
/// world type.
pub(crate) struct Editor {
    pub(crate) palette: style::Palette,
    pub(crate) command_modifier: puri::keyboard::CommandModifier,
    pub(crate) computations: computations::Computations,
    pub(crate) timers: timers::Timers,
    pub(crate) focused: bool,
    /// This window draws its own menu bar (the drawn menu system).
    pub(crate) drawn_menu: bool,
    /// File commands may ask for a path through the platform's dialogs; a
    /// headless session has none, so its untitled saves do nothing.
    #[cfg_attr(
        target_arch = "wasm32",
        expect(dead_code, reason = "the browser saves by download")
    )]
    pub(crate) file_dialogs: bool,
    pub(crate) state: RenderState,
    /// Multiplies the display's scale, like a browser's page zoom.
    pub(crate) zoom: f64,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) paint_resources: Resources,
    pub(crate) font_cx: FontContext,
    pub(crate) layout_cx: LayoutContext<Brush>,
    pub(crate) text_clipboard: Clipboard,
    /// The last structural copy, until the shell puts a picture of it
    /// beside it.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) unpictured: Option<crate::picture::Copied>,
    pub(crate) text_cache: puri::text::TextCache,
    pub(crate) stack: stack::Stack,
    pub(crate) model: Model,
    /// Where the document lives; `None` is untitled until the first
    /// save asks for a path.
    pub(crate) doc_path: Option<PathBuf>,
    pub(crate) shown_title: Option<ShownTitle>,
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
    pub(crate) gesture: Option<gesture::Active>,
    pub(crate) reducer: WindowEventReducer,
    /// Carries the application commands the drawn menu chooses to the app.
    pub(crate) proxy: Option<winit::event_loop::EventLoopProxy<UserEvent>>,
    pub(crate) pending_discard: Option<AfterDiscard>,
}

/// What a window last showed of its document, so it's told only of changes.
#[derive(PartialEq)]
pub(crate) struct ShownTitle {
    window: WindowId,
    title: String,
    edited: bool,
    path: Option<PathBuf>,
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
    /// event's implicit cursor state (see
    /// https://github.com/rust-windowing/winit/issues/942 and
    /// https://github.com/rust-windowing/winit/pull/1490). Treating that
    /// synthetic refresh as a barrier would let projection work keep AppKit
    /// from reaching its redraw boundary under continuous input.
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
        let next = self.cursor();
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

    /// The running gesture's cursor, else the one the hovered target asked
    /// for.
    pub(crate) fn cursor(&self) -> CursorIcon {
        let cursor = self
            .editor
            .gesture
            .as_ref()
            .and_then(|gesture| gesture.cursor())
            .or_else(|| {
                let hovered = self.frame.hover.as_ref()?;
                let dispatch = &self.frame.dispatch;
                dispatch
                    .hover_geometry
                    .cursor(dispatch.pointer_root.as_ref(), hovered)
            });
        match cursor {
            None => CursorIcon::Default,
            Some(Cursor::ColumnResize) => CursorIcon::ColResize,
            Some(Cursor::RowResize) => CursorIcon::RowResize,
            Some(Cursor::HorizontalDrag) => CursorIcon::EwResize,
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
            self.refresh_frame(self.editor.scale(&window), viewport(&window));
            self.sync_window(&window);
            window.request_redraw();
        }
    }
}

pub(crate) fn new_editor(
    palette: style::Palette,
    command_modifier: puri::keyboard::CommandModifier,
    drawn_menu: bool,
    stack: stack::Stack,
    font_cx: FontContext,
    doc: gid::Document,
    doc_path: Option<PathBuf>,
    text_binders: gid_text::Binders,
    proxy: Option<winit::event_loop::EventLoopProxy<UserEvent>>,
) -> Editor {
    #[cfg(all(target_arch = "wasm32", target_feature = "atomics"))]
    let computations = proxy
        .as_ref()
        .map(|_| computations::Computations::new(web_worker::executor(), web_worker::wake))
        .unwrap_or_default();
    // Without shared memory, a browser page has no worker to run jobs on.
    #[cfg(all(target_arch = "wasm32", not(target_feature = "atomics")))]
    let computations = proxy
        .as_ref()
        .map(|_| computations::Computations::without_tasks())
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
        file_dialogs: true,
        state: RenderState::Suspended(None),
        zoom: 1.0,
        #[cfg(not(target_arch = "wasm32"))]
        paint_resources: Resources::default(),
        font_cx,
        layout_cx: LayoutContext::new(),
        text_cache: puri::text::TextCache::default(),
        #[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
        text_clipboard: Clipboard::System,
        #[cfg(any(test, target_arch = "wasm32"))]
        text_clipboard: Clipboard::default(),
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        unpictured: None,
        stack,
        model: Model::new(doc),
        doc_path,
        shown_title: None,
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
    /// The editor's capabilities for one dispatch.
    pub(crate) fn session(&mut self) -> crate::editing::Session<'_> {
        crate::editing::Session {
            model: &mut self.model,
            libraries: &self.stack.libraries,
            gesture: &mut self.gesture,
            timers: &mut self.timers,
            text: crate::editing::TextTools {
                fonts: &mut self.font_cx,
                layouts: &mut self.layout_cx,
                clipboard: &mut self.text_clipboard,
            },
        }
    }
    pub(crate) fn window_id(&self) -> Option<WindowId> {
        self.window().map(|window| window.id())
    }

    pub(crate) fn window(&self) -> Option<Arc<Window>> {
        match &self.state {
            RenderState::Active { window, .. } => Some(window.clone()),
            RenderState::Suspended(window) => window.clone(),
        }
    }

    /// Physical pixels per logical point in `window`, zoom included.
    pub(crate) fn scale(&self, window: &Window) -> f64 {
        window.scale_factor() * self.zoom
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn next_zoom(&self, larger: bool) -> Option<f64> {
        const ZOOMS: [f64; 13] = [
            0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0,
        ];
        if larger {
            ZOOMS.into_iter().find(|zoom| *zoom > self.zoom)
        } else {
            ZOOMS.into_iter().rev().find(|zoom| *zoom < self.zoom)
        }
    }

    /// Advance the running gesture along `samples`; whether one is running.
    pub(crate) fn advance_gesture(&mut self, samples: &[Point]) -> bool {
        let advanced = self.session().advance_gesture(samples);
        if advanced == Some(true) {
            self.refresh_title();
        }
        advanced.is_some()
    }

    pub(crate) fn finish_gesture(&mut self) -> bool {
        self.session().finish_gesture()
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

    /// Show the document's title, and on macOS whether it's edited and where
    /// it lives, if they changed. The shell calls this after dispatch too,
    /// since the edits handlers make can't reach the window.
    pub(crate) fn refresh_title(&mut self) {
        if let Some(window) = self.window() {
            let shown = ShownTitle {
                window: window.id(),
                title: self.title(),
                edited: self.model.dirty(),
                path: self.doc_path.clone(),
            };
            if self.shown_title.as_ref() != Some(&shown) {
                window.set_title(&shown.title);
                #[cfg(target_os = "macos")]
                {
                    use winit::platform::macos::WindowExtMacOS;
                    window.set_document_edited(shown.edited);
                    macos_window::set_represented(&window, shown.path.as_deref());
                }
                self.shown_title = Some(shown);
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
            shown: area.shown.clone(),
            preset: self.stack.preset(&area.shown),
        }
    }

    pub(crate) fn menu_availability(&self) -> command::Availability {
        let selected_root = self
            .model
            .selection
            .as_ref()
            .map(selection::Selection::root);
        command::Availability {
            #[cfg(any(target_os = "macos", target_os = "linux", target_arch = "wasm32"))]
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
            close_pane: selected_root
                .is_some_and(|root| matches!(root.target(), Target::Pane { .. })),
            show_panes: workspace::can_open(self.model.doc.root.as_ref()),
            move_up: selected_root
                .is_some_and(|root| self.model.workspace.can_move(root, workspace::Move::Up)),
            move_down: selected_root
                .is_some_and(|root| self.model.workspace.can_move(root, workspace::Move::Down)),
            move_left: selected_root
                .is_some_and(|root| self.model.workspace.can_move(root, workspace::Move::Left)),
            move_right: selected_root
                .is_some_and(|root| self.model.workspace.can_move(root, workspace::Move::Right)),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            actual_size: self.zoom != 1.0,
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            zoom_in: self.next_zoom(true).is_some(),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            zoom_out: self.next_zoom(false).is_some(),
        }
    }

    /// Opens the selection in a pane on `side`: the place it's stored,
    /// anchored at its nearest cell, so the pane shows and edits that place,
    /// or for a computed value, which has no place, a copy of it.
    fn open_selected_in_pane(&mut self, side: workspace::Side) {
        self.open_pane(side, |editor, wrapped| {
            let selection = editor.model.selection.as_ref()?;
            let sources = editor.sources();
            let place = selection.location().and_then(|location| {
                location
                    .stored(&sources)
                    .map(crate::location::Location::into_owned)
            });
            Some(match place {
                Some(mut place) => {
                    // Wrapping moves every path from the root under `main`.
                    if wrapped && place.start == crate::location::Start::Root {
                        place.path = std::iter::once(gid::Step::Key(workspace::vocabulary::MAIN))
                            .chain(place.path.iter().cloned())
                            .collect();
                    }
                    crate::location::value(&place)
                }
                None => selection.value(&sources)?.clone(),
            })
        });
    }

    /// Shows the workspace's panes declaration in a pane on the right, as a
    /// location, so editing it there edits the panes; selects the pane
    /// already showing it, if one does.
    fn show_panes(&mut self) {
        let entry = crate::location::value(&crate::location::Location::root(vec![gid::Step::Key(
            workspace::vocabulary::PANES,
        )]));
        let sources = self.sources();
        let existing = [workspace::Side::Left, workspace::Side::Right]
            .into_iter()
            .flat_map(|side| self.model.workspace.column(side).panes.iter())
            .find_map(|pane| match pane.view.root.target() {
                Target::Pane { path } if sources.resolve_path(path) == Some(&entry) => Some((
                    pane.view.root.clone(),
                    workspace::location_path(&entry, path)?,
                )),
                _ => None,
            });
        match existing {
            Some((root, shown)) => {
                self.model.selection = Some(selection::Selection::edge(
                    &root,
                    crate::occurrence::of(&shown),
                ));
            }
            None => self.open_pane(workspace::Side::Right, |_, _| Some(entry)),
        }
    }

    /// Opens what `entry` gives as the last pane on `side` and selects what
    /// it shows. A bare root is wrapped in a workspace first; `entry` learns
    /// whether it was, since wrapping moves every path from the root under
    /// `main`.
    fn open_pane(
        &mut self,
        side: workspace::Side,
        entry: impl FnOnce(&Self, bool) -> Option<gid::Value>,
    ) {
        let next = self.model.doc.root.as_ref().and_then(|root| {
            let (root, wrapped) = workspace::workspace_root(root);
            let entry = entry(self, wrapped)?;
            let (root, path) = workspace::append(&root, side, entry.clone())?;
            let shown = workspace::location_path(&entry, &path).unwrap_or(path);
            Some((root, shown, wrapped))
        });
        if let Some((value, shown, wrapped)) = next {
            let before = self.model.snapshot();
            Rc::make_mut(&mut self.model.doc).root = Some(value);
            self.model.history.record(before);
            if wrapped {
                let document = &mut self.model.workspace.document;
                document.annotations = document
                    .annotations
                    .under(&[crate::occurrence::Step::Key(workspace::vocabulary::MAIN)]);
            }
            self.model
                .workspace
                .sync_declared(&workspace::declarations(self.model.doc.root.as_ref()));
            // The value opens as its column's last pane.
            self.model.selection = self.model.workspace.column(side).panes.last().map(|pane| {
                selection::Selection::edge(&pane.view.root, crate::occurrence::of(&shown))
            });
            self.refresh_title();
        }
    }

    /// Closes the selected pane by removing its entry, so a pane showing a
    /// location leaves that place as it is.
    fn close_selected_pane(&mut self) {
        let next =
            self.model
                .selection
                .as_ref()
                .and_then(|selection| match selection.root().target() {
                    Target::Pane { path } => {
                        crate::spine::without(self.model.doc.root.as_ref()?, path)
                    }
                    Target::Document => None,
                });
        if let Some(value) = next {
            let before = self.model.snapshot();
            Rc::make_mut(&mut self.model.doc).root = Some(value);
            self.model.history.record(before);
            self.model.selection = None;
            self.model
                .workspace
                .sync_declared(&workspace::declarations(self.model.doc.root.as_ref()));
            self.refresh_title();
        }
    }

    fn move_selected_pane(&mut self, direction: workspace::Move) {
        let next =
            self.model
                .selection
                .as_ref()
                .and_then(|selection| match selection.root().target() {
                    Target::Pane { path } => {
                        // A pane showing a place from the root keeps its
                        // selection's path; one under the declaration moves
                        // with it.
                        let suffix = selection
                            .path()
                            .strip_prefix(crate::occurrence::of(path).as_slice())
                            .map(<[_]>::to_vec);
                        let (value, path) =
                            workspace::move_value(self.model.doc.root.as_ref()?, path, direction)?;
                        Some((value, path, suffix, selection.root().clone()))
                    }
                    Target::Document => None,
                });
        if let Some((value, path, suffix, root)) = next {
            let before = self.model.snapshot();
            Rc::make_mut(&mut self.model.doc).root = Some(value);
            self.model.history.record(before);
            let next_root = Root::pane(path.clone());
            if let Some(view) = self.model.workspace.view_mut(&root) {
                view.root = next_root.clone();
                view.annotations = Default::default();
            }
            if let Some(selection) = self.model.selection.as_mut() {
                let path = match suffix {
                    Some(suffix) => crate::occurrence::of(&path)
                        .into_iter()
                        .chain(suffix)
                        .collect(),
                    None => selection.path().to_vec(),
                };
                selection.relocate(next_root, path);
            }
            self.model
                .workspace
                .sync_declared(&workspace::declarations(self.model.doc.root.as_ref()));
            self.refresh_title();
        }
    }

    /// The view the selection is in, which view commands act on.
    fn selected_view(&self) -> Option<Root> {
        self.model
            .selection
            .as_ref()
            .map(selection::Selection::root)
            .cloned()
    }

    pub(crate) fn choose_menu(&mut self, command: Command, geometry: Geometry<'_, Editor>) {
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

    /// A document command against this editor. It only changes the editor;
    /// its caller schedules the next frame, as a dispatch and
    /// `App::run_command` do.
    pub(crate) fn run_doc_command(&mut self, command: DocCommand, geometry: Geometry<'_, Editor>) {
        match command {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            DocCommand::Save => self.desktop_save(false),
            #[cfg(target_arch = "wasm32")]
            DocCommand::Save => self.web_save(false),
            #[cfg(target_arch = "wasm32")]
            DocCommand::SaveAs => self.web_save(true),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            DocCommand::SaveAs => self.desktop_save(true),
            DocCommand::Undo => self.step_history(true, geometry),
            DocCommand::Redo => self.step_history(false, geometry),
            DocCommand::OpenPane(side) => self.open_selected_in_pane(side),
            DocCommand::ClosePane => self.close_selected_pane(),
            DocCommand::ShowPanes => self.show_panes(),
            DocCommand::MovePane(direction) => self.move_selected_pane(direction),
            DocCommand::Preset(preset) => {
                let selected = self.selected_view();
                let showing = self.stack.preset(
                    &self
                        .model
                        .workspace
                        .selected_or_document(selected.as_ref())
                        .shown,
                );
                let preset = if showing == Some(preset) {
                    crate::stack::Preset::All
                } else {
                    preset
                };
                self.model.workspace.show(selected.as_ref(), preset.shown());
            }
            DocCommand::Projection(library) => self
                .model
                .workspace
                .toggle_library(self.selected_view().as_ref(), library),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            DocCommand::ActualSize => self.zoom = 1.0,
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            DocCommand::ZoomIn | DocCommand::ZoomOut => {
                if let Some(zoom) = self.next_zoom(command == DocCommand::ZoomIn) {
                    self.zoom = zoom;
                }
            }
        }
    }

    pub(crate) fn menu_key(
        &mut self,
        event: &KeyboardEvent,
        geometry: Geometry<'_, Editor>,
    ) -> bool {
        let availability = self.menu_availability();
        let navigation = match menu::shortcut(event, self.command_modifier)
            .filter(|command| availability.enabled(*command))
        {
            Some(command) => menu::Navigation::Activate(command),
            None => menu::navigate(
                &mut self.menu,
                &menu::definition(self.stack.switchable().map(|(library, _)| library)),
                availability,
                event,
            ),
        };
        match navigation {
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
    pub(crate) fn step_history(&mut self, back: bool, geometry: Geometry<'_, Editor>) {
        self.finish_gesture();
        if self.model.step_history(back) {
            geometry.reveal_selection(&mut self.model);
            self.refresh_title();
        }
    }

    /// Save saves in place, or asks for a path when untitled; save-as
    /// always asks. Write-through editing means the GID document is always
    /// current, so there is nothing to flush first. A cancelled dialog
    /// saves nothing.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn desktop_save(&mut self, save_as: bool) {
        let in_place = (!save_as).then(|| self.doc_path.clone()).flatten();
        let target = in_place.or_else(|| {
            self.file_dialogs
                .then(|| {
                    let file_name = self
                        .doc_path
                        .as_deref()
                        .and_then(|path| path.file_name())
                        .map_or_else(
                            || "untitled.gid".to_owned(),
                            |name| name.to_string_lossy().into_owned(),
                        );
                    text_dialog().set_file_name(file_name).save_file()
                })
                .flatten()
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

    /// The browser's save is a download of the document's text. As on the
    /// desktop, an untitled document asks for its name, which then sticks;
    /// save-as always asks. A cancelled prompt saves nothing.
    #[cfg(target_arch = "wasm32")]
    fn web_save(&mut self, save_as: bool) {
        let current = self
            .doc_path
            .as_deref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned());
        let name = match (save_as, current) {
            (false, Some(name)) => Some(name),
            (_, current) => crate::web::ask_name(current.as_deref().unwrap_or("untitled.gid")),
        };
        if let Some(name) = name {
            match crate::web::download(&name, &gid_text::print(&self.model.doc, &self.text_binders))
            {
                Ok(()) => {
                    self.model.mark_saved();
                    self.finish_gesture();
                    self.adopt_doc_path(PathBuf::from(name));
                }
                Err(error) => web_sys::console::error_1(&error),
            }
        }
    }

    /// Libraries decide how everything is drawn and what cells resolve to,
    /// so a new stack keeps the document, selection, and folds but discards
    /// gestures and memoized work computed under the old one.
    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn replace_stack(&mut self, stack: stack::Stack) {
        self.finish_gesture();
        self.computations.reset();
        self.stack = stack;
    }

    /// Replace document-owned state, retaining the window and its platform
    /// resources. In particular, the input reducer still knows the physical
    /// pointer position and held modifiers; old gestures do not survive.
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
            file_dialogs: _,
            state: _,
            zoom: _,
            #[cfg(not(target_arch = "wasm32"))]
            paint_resources,
            font_cx: _,
            layout_cx: _,
            text_clipboard: _,
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            unpictured,
            text_cache: _,
            stack: _,
            model,
            doc_path,
            shown_title: _,
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
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            *unpictured = None;
        }
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

    fn adopt_doc_path(&mut self, path: PathBuf) {
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
