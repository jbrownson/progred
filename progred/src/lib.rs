//! Window shell: translate platform input, schedule updates, and present
//! the editor frame through Vello or Canvas2D.

mod annotations;
mod command;
mod commands;
mod completion;
mod computations;
mod display;
mod editing;
mod filter;
mod frame;
mod gesture;
mod gid_text;
#[cfg(target_os = "ios")]
mod gpu;
#[cfg(test)]
mod grap_examples;
mod history;
mod hover;
mod identity;
mod input;
mod libraries;
#[cfg(target_os = "macos")]
mod macos_surface;
#[cfg(target_os = "macos")]
mod macos_window;
mod menu;
mod model;
mod modifiers;
#[cfg(target_os = "macos")]
mod native_menu;
mod navigate;
mod placed;
mod platform;
mod projection;
mod render;
#[cfg(test)]
mod sample;
mod selection;
mod site;
mod sources;
mod spine;
mod stack;
mod styles;
#[cfg(test)]
mod test_values;
mod text_store;
#[cfg(target_arch = "wasm32")]
pub mod web_render;
#[cfg(target_arch = "wasm32")]
pub mod web_worker;
mod workspace;

#[cfg(all(feature = "cam-profile", target_arch = "wasm32"))]
pub use libraries::fidget::mesh::performance::take_profile_mesh;
#[cfg(feature = "cam-profile")]
pub use libraries::toolpath::performance::profile_cam;

use crate::command::{AppCommand, Command, DocCommand};
#[cfg(not(target_arch = "wasm32"))]
use crate::frame::Paint;
use crate::frame::{FrameState, Hovered};
use crate::model::Model;
use kurbo::{Point, Rect, Size};
use peniko::{Brush, Color};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

#[cfg(target_os = "ios")]
use gpu::{RenderContext, RenderSurface};
use parley::{FontContext, LayoutContext};
use puri::edit::TextClipboard;
use puri::handler::ImeEvent;
#[cfg(not(target_arch = "wasm32"))]
use puri_vello::compositor::{Compositor, Resources};
#[cfg(test)]
use ui_events::ScrollDelta;
use ui_events::keyboard::{Key, KeyboardEvent, Modifiers, NamedKey};
use ui_events::pointer::{
    PointerEvent, PointerGestureEvent, PointerId, PointerInfo, PointerScrollEvent, PointerType,
    PointerUpdate,
};
use ui_events_winit::{WindowEventReducer, WindowEventTranslation};
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "ios")))]
use vello::util::{RenderContext, RenderSurface};
#[cfg(not(target_arch = "wasm32"))]
use vello::wgpu::{self, CurrentSurfaceTexture};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;
use winit::application::ApplicationHandler;
#[cfg(not(target_arch = "wasm32"))]
use winit::dpi::LogicalSize;
use winit::event::{Ime, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
#[cfg(target_arch = "wasm32")]
use winit::platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys, WindowExtWebSys};
#[cfg(target_os = "linux")]
use winit::platform::x11::WindowAttributesExtX11;
use winit::window::{CursorIcon, Window, WindowId};

/// Everything arriving through the event-loop proxy.
pub(crate) enum UserEvent {
    ComputationFinished,
    #[cfg(target_os = "macos")]
    NativeMenu(native_menu::Event),
    Command(Command),
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Discard {
        window: WindowId,
        accepted: bool,
    },
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
    #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
    Quit,
}

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

/// The pasteboard type structural copies ride under, beside their
/// plain text; its PRESENCE is the structure/text distinction, so
/// text that merely spells Value JSON is never mistaken for a copy.
#[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
pub(crate) const CLIPBOARD_FORMAT: &str = "com.progred.value";

#[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
pub(crate) struct SystemTextClipboard;

#[cfg(any(test, target_arch = "wasm32", target_os = "ios"))]
#[derive(Default)]
pub(crate) struct SystemTextClipboard {
    text: Option<String>,
    structure: Option<gid::Value>,
}

#[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
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

#[cfg(any(test, target_arch = "wasm32", target_os = "ios"))]
impl TextClipboard for SystemTextClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.text.clone()
    }

    fn set_text(&mut self, text: &str) {
        self.text = Some(text.to_string());
        self.structure = None;
    }
}

pub(crate) struct PendingPaint {
    pub(crate) scale: f64,
    pub(crate) viewport: Size,
    pub(crate) renders: Vec<placed::Render>,
}

type PendingScroll = PendingBatch<PointerScrollEvent>;
type PendingGesture = PendingBatch<PointerGestureEvent>;

struct PendingBatch<T> {
    events: Vec<T>,
    scale: f64,
    viewport: Size,
}

struct PendingPointer {
    event: PointerUpdate,
    start: Point,
    scale: f64,
    viewport: Size,
}

impl PendingPointer {
    fn merge(&mut self, mut next: Self) -> Result<(), Self> {
        if self.scale == next.scale
            && self.viewport == next.viewport
            && self.event.pointer == next.event.pointer
            && self.event.current.buttons == next.event.current.buttons
            && self.event.current.modifiers == next.event.current.modifiers
        {
            self.event.coalesced.push(std::mem::replace(
                &mut self.event.current,
                next.event.current,
            ));
            self.event.coalesced.append(&mut next.event.coalesced);
            self.event.predicted = next.event.predicted;
            Ok(())
        } else {
            Err(next)
        }
    }
}

fn continuous_input(event: &WindowEvent) -> bool {
    matches!(
        event,
        WindowEvent::MouseWheel { .. }
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::PinchGesture {
                phase: winit::event::TouchPhase::Moved,
                ..
            }
            | WindowEvent::RotationGesture {
                phase: winit::event::TouchPhase::Moved,
                ..
            }
            | WindowEvent::Touch(winit::event::Touch {
                phase: winit::event::TouchPhase::Moved,
                ..
            })
    )
}

impl<T> PendingBatch<T> {
    fn merge(&mut self, mut next: Self) -> Result<(), Self> {
        if self.scale == next.scale && self.viewport == next.viewport {
            self.events.append(&mut next.events);
            Ok(())
        } else {
            Err(next)
        }
    }
}

/// Process-wide state: the GPU, the shared caches, and the editors.
pub(crate) struct App {
    #[cfg(target_arch = "wasm32")]
    web_renderer: web_render::Renderer,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) context: RenderContext,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) renderers: Vec<Option<Compositor>>,
    /// Editor configuration shared by every document loaded into the
    /// app: library cells, Rust functions, and composed projection.
    /// Each editor holds its own (cheap) clone, so a window can later
    /// filter or extend its libraries independently; this master copy
    /// seeds new editors.
    #[cfg_attr(any(target_arch = "wasm32", target_os = "ios"), allow(dead_code))]
    pub(crate) stack: stack::Stack<Editor>,
    /// The font database master; editors hold cheap clones over the
    /// same shared font data.
    #[cfg_attr(any(target_arch = "wasm32", target_os = "ios"), allow(dead_code))]
    pub(crate) fonts: FontContext,
    #[cfg(target_os = "macos")]
    pub(crate) native_menu: native_menu::Menu,
    #[cfg_attr(any(target_arch = "wasm32", target_os = "ios"), allow(dead_code))]
    pub(crate) proxy: winit::event_loop::EventLoopProxy<UserEvent>,
    /// New windows draw the in-window menu system.
    pub(crate) drawn_menu: bool,
    pub(crate) editors: Vec<EditorRunner>,
    /// The window whose editor application-level commands target.
    pub(crate) focused: Option<WindowId>,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) quit: QuitState,
    /// AppKit's running cascade origin for windows without a saved
    /// frame.
    #[cfg(target_os = "macos")]
    pub(crate) cascade: macos_window::CascadePoint,
}

/// Quit reviews windows one at a time, focused first, each through its
/// discard sheet. Only declining a sheet the chain itself presented
/// abandons the quit.
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuitState {
    Idle,
    Draining { awaiting: Option<WindowId> },
}

/// One window editing one document: its own CellId universe, model,
/// history, interaction state, and measurement caches (the font
/// context is a cheap clone over shared font data). The dispatch
/// world type.
pub(crate) struct Editor {
    pub(crate) computations: computations::Computations,
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
    gesture: Option<gesture::Active<Editor>>,
    pub(crate) reducer: WindowEventReducer,
    /// Routes the discard sheet's answer back into the loop.
    #[cfg_attr(any(target_arch = "wasm32", target_os = "ios"), allow(dead_code))]
    pub(crate) proxy: Option<winit::event_loop::EventLoopProxy<UserEvent>>,
    pub(crate) pending_discard: Option<AfterDiscard>,
}

/// Per-window execution state. Widgets receive only `editor`, never this runner.
pub(crate) struct EditorRunner {
    pub(crate) editor: Editor,
    pub(crate) frame: FrameState,
    cursor_icon: CursorIcon,
    /// Consecutive scroll packets are a batch of observed samples.
    /// Hold them until paint or another event establishes an ordering
    /// boundary, then dispatch the batch through the retained frame.
    pending_scroll: Option<PendingScroll>,
    pending_gesture: Option<PendingGesture>,
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
    pending_pointer: Option<PendingPointer>,
}

impl EditorRunner {
    fn new(editor: Editor) -> Self {
        Self {
            editor,
            frame: FrameState::default(),
            cursor_icon: CursorIcon::Default,
            pending_scroll: None,
            pending_gesture: None,
            pending_pointer: None,
        }
    }

    /// Flush before discrete input. A paint request already presents the
    /// resulting frame, so only other events need a future redraw requested.
    fn flush_before_window_event(&mut self, event: &WindowEvent) -> bool {
        !continuous_input(event)
            && self.flush_pending_continuous()
            && !matches!(event, WindowEvent::RedrawRequested)
    }

    fn sync_cursor(&mut self, window: &Window) {
        let next = cursor_icon(self.frame.hover.as_ref());
        if next != self.cursor_icon {
            window.set_cursor(next);
            self.cursor_icon = next;
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
            self.sync_cursor(&window);
            window.request_redraw();
        }
    }
}

/// One canonical spelling per document, so every identity — frame
/// memory, the duplicate-window rule — agrees regardless of how the
/// path was written.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn canonical(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

pub(crate) fn menu_height(drawn_menu: bool, scale: f64) -> f64 {
    if drawn_menu {
        menu::bar_height(scale)
    } else {
        0.0
    }
}

pub(crate) fn content_viewport(drawn_menu: bool, viewport: Size, scale: f64) -> Rect {
    Rect::new(
        0.0,
        menu_height(drawn_menu, scale).min(viewport.height),
        viewport.width,
        viewport.height,
    )
}

fn new_editor(
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
        computations,
        drawn_menu,
        state: RenderState::Suspended(None),
        #[cfg(not(target_arch = "wasm32"))]
        paint_resources: Resources::default(),
        font_cx,
        layout_cx: LayoutContext::new(),
        text_cache: puri::text::TextCache::default(),
        #[cfg(all(not(test), any(target_os = "macos", target_os = "linux")))]
        text_clipboard: SystemTextClipboard,
        #[cfg(any(test, target_arch = "wasm32", target_os = "ios"))]
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

fn font_context() -> FontContext {
    #[cfg(not(target_arch = "wasm32"))]
    return FontContext::new();
    #[cfg(target_arch = "wasm32")]
    {
        use parley::fontique::Blob;
        use parley::style::GenericFamily;

        let mut fonts = FontContext::new();
        let sans = fonts
            .collection
            .register_fonts(
                Blob::from(include_bytes!("../assets/NotoSans-Regular.ttf").to_vec()),
                None,
            )
            .into_iter()
            .map(|(family, _)| family)
            .collect::<Vec<_>>();
        let mono = fonts
            .collection
            .register_fonts(
                Blob::from(include_bytes!("../assets/NotoSansMono-Regular.ttf").to_vec()),
                None,
            )
            .into_iter()
            .map(|(family, _)| family)
            .collect::<Vec<_>>();
        fonts
            .collection
            .set_generic_families(GenericFamily::SystemUi, sans.iter().copied());
        fonts
            .collection
            .set_generic_families(GenericFamily::SansSerif, sans.into_iter());
        fonts
            .collection
            .set_generic_families(GenericFamily::Monospace, mono.into_iter());
        fonts
    }
}

fn translate_window_event(
    reducer: &mut WindowEventReducer,
    scale: f64,
    event: &WindowEvent,
) -> Option<WindowEventTranslation> {
    if matches!(event, WindowEvent::Focused(false)) {
        // Focus loss can take the release away from this window. Departure alone
        // does not cancel: Winit forwards macOS drags outside the client area.
        *reducer = WindowEventReducer::default();
        Some(WindowEventTranslation::Pointer(PointerEvent::Cancel(
            PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
        )))
    } else {
        reducer.reduce(scale, event)
    }
}

fn cursor_icon(hover: Option<&Hovered>) -> CursorIcon {
    match hover {
        Some(Hovered::Divider(workspace::Divider::Columns(_))) => CursorIcon::ColResize,
        Some(Hovered::Divider(workspace::Divider::Panes { .. })) => CursorIcon::RowResize,
        _ => CursorIcon::Default,
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn text_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("GID", &["gid"])
}

impl ApplicationHandler<UserEvent> for App {
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        for runner in &mut self.editors {
            if runner.flush_pending_continuous()
                && let RenderState::Active { window, .. } = &runner.editor.state
            {
                window.request_redraw();
            }
        }
        match event {
            UserEvent::ComputationFinished => {
                for runner in &mut self.editors {
                    if runner.editor.computations.tasks.poll()
                        && let RenderState::Active { window, .. } = &runner.editor.state
                    {
                        let window = window.clone();
                        let size = window.inner_size();
                        runner.refresh_frame(
                            window.scale_factor(),
                            Size::new(f64::from(size.width), f64::from(size.height)),
                        );
                        runner.sync_cursor(&window);
                        window.request_redraw();
                    }
                }
            }
            #[cfg(target_os = "macos")]
            UserEvent::NativeMenu(event) => {
                if let Some(command) = self.native_menu.command(&event) {
                    self.run_command(event_loop, command);
                }
            }
            UserEvent::Command(command) => self.run_command(event_loop, command),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            UserEvent::Discard { window, accepted } => {
                #[cfg(any(target_os = "macos", target_os = "linux"))]
                if self.quit
                    == (QuitState::Draining {
                        awaiting: Some(window),
                    })
                {
                    self.quit = if accepted {
                        QuitState::Draining { awaiting: None }
                    } else {
                        QuitState::Idle
                    };
                }
                let Some(index) = self.editor_index(window) else {
                    return;
                };
                let pending = self.editors[index].editor.pending_discard.take();
                if accepted && let Some(then) = pending {
                    self.proceed(event_loop, index, then);
                }
            }
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // After launch, so winit cannot replace it (its own default
        // menu is disabled at loop construction).
        #[cfg(target_os = "macos")]
        self.native_menu.install();

        for index in 0..self.editors.len() {
            self.resume_editor(event_loop, index);
        }
        if self.focused.is_none() {
            self.focused = self
                .editors
                .first()
                .and_then(|runner| runner.editor.window_id());
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        for runner in &mut self.editors {
            runner.flush_pending_continuous();
            #[cfg(not(target_arch = "wasm32"))]
            {
                runner.editor.paint_resources = Resources::default();
            }
            if let RenderState::Active { window, .. } = &runner.editor.state {
                runner.editor.state = RenderState::Suspended(Some(window.clone()));
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::Focused(true) = &event {
            self.focused = Some(window_id);
            if let Some(index) = self.editor_index(window_id) {
                self.sync_menus(index);
            }
        }
        let Some(index) = self.editor_index(window_id) else {
            return;
        };
        self.editor_window_event(event_loop, index, window_id, event);
    }
}

impl App {
    fn editor_index(&self, id: WindowId) -> Option<usize> {
        self.editors
            .iter()
            .position(|runner| runner.editor.window_id() == Some(id))
    }

    /// The editor application-level commands act on: the focused
    /// window's, or the sole survivor's while focus is unknown.
    fn focused_index(&self) -> Option<usize> {
        self.focused
            .and_then(|id| self.editor_index(id))
            .or_else(|| (!self.editors.is_empty()).then_some(0))
    }

    /// Opens a document in its own new window — every document lives
    /// in exactly one window.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn open_editor(
        &mut self,
        event_loop: &ActiveEventLoop,
        doc: gid::Document,
        path: Option<PathBuf>,
        binders: gid_text::Binders,
    ) {
        self.editors.push(EditorRunner::new(new_editor(
            self.drawn_menu,
            self.stack.clone(),
            self.fonts.clone(),
            doc,
            path,
            binders,
            Some(self.proxy.clone()),
        )));
        self.resume_editor(event_loop, self.editors.len() - 1);
    }

    /// Removes one window. The surface and window close on drop. The
    /// last close quits where that is the platform convention, and
    /// always quits while the quit chain is draining.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn close_editor(&mut self, event_loop: &ActiveEventLoop, index: usize) {
        let closed = self.editors.remove(index);
        if closed.editor.window_id().is_some() && closed.editor.window_id() == self.focused {
            self.focused = None;
        }
        drop(closed);
        if self.editors.is_empty() {
            if platform::QUITS_ON_LAST_CLOSE && self.quit == QuitState::Idle {
                event_loop.exit();
            }
            // The resident menu bar grays every document command.
            #[cfg(target_os = "macos")]
            self.native_menu.sync(None);
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn begin_quit(&mut self, event_loop: &ActiveEventLoop) {
        if self
            .editors
            .iter()
            .all(|runner| runner.editor.pending_discard.is_none())
        {
            self.quit = QuitState::Draining { awaiting: None };
            self.advance_quit(event_loop);
        }
    }

    /// Closes clean windows until one needs its discard sheet; the
    /// answer re-enters through [`UserEvent::Discard`] and advances
    /// again. An empty list ends the process.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn advance_quit(&mut self, event_loop: &ActiveEventLoop) {
        while self.quit == (QuitState::Draining { awaiting: None }) {
            if self.editors.is_empty() {
                event_loop.exit();
                return;
            }
            let index = self.focused_index().unwrap_or(0);
            if self.editors[index].editor.model.dirty() {
                if let RenderState::Active { window, .. } = &self.editors[index].editor.state {
                    window.focus_window();
                    self.quit = QuitState::Draining {
                        awaiting: Some(window.id()),
                    };
                }
                self.request_discard(event_loop, index, AfterDiscard::CloseWindow);
                return;
            }
            self.close_editor(event_loop, index);
        }
    }

    fn resume_editor(&mut self, event_loop: &ActiveEventLoop, index: usize) {
        #[cfg(target_os = "macos")]
        let App {
            editors,
            context,
            renderers,
            cascade,
            ..
        } = &mut *self;
        #[cfg(all(not(target_arch = "wasm32"), not(target_os = "macos")))]
        let App {
            editors,
            context,
            renderers,
            ..
        } = &mut *self;
        #[cfg(target_arch = "wasm32")]
        let App { editors, .. } = &mut *self;
        let runner = &mut editors[index];
        let RenderState::Suspended(cached_window) = &mut runner.editor.state else {
            return;
        };

        let window = cached_window.take().unwrap_or_else(|| {
            let attributes = Window::default_attributes().with_title(runner.editor.title());
            #[cfg(not(target_arch = "wasm32"))]
            let attributes = attributes.with_inner_size(LogicalSize::new(900, 640));
            // The app id must match linux/progred.desktop for compositors
            // to associate the window with the desktop entry. Wayland and
            // X11 read the same attribute.
            #[cfg(target_os = "linux")]
            let attributes = attributes.with_name("progred", "progred");
            #[cfg(target_arch = "wasm32")]
            let attributes = {
                let canvas = web_sys::window()
                    .and_then(|window| window.document())
                    .and_then(|document| document.get_element_by_id("progred"))
                    .and_then(|element| element.dyn_into::<HtmlCanvasElement>().ok())
                    .expect("#progred canvas");
                attributes
                    .with_canvas(Some(canvas))
                    .with_prevent_default(true)
            };
            let window = event_loop.create_window(attributes).unwrap();
            #[cfg(target_os = "macos")]
            {
                macos_window::place_and_autosave_frame(
                    &window,
                    runner.editor.doc_path.as_deref(),
                    cascade,
                );
                macos_window::set_represented(&window, runner.editor.doc_path.as_deref());
            }
            Arc::new(window)
        });

        #[cfg(not(target_arch = "wasm32"))]
        {
            let size = window.inner_size();
            #[cfg(target_os = "macos")]
            let surface_future = context.create_render_surface(
                macos_surface::create(&context.instance, &window),
                size.width,
                size.height,
                wgpu::PresentMode::AutoVsync,
            );
            #[cfg(not(target_os = "macos"))]
            let surface_future = context.create_surface(
                window.clone(),
                size.width,
                size.height,
                wgpu::PresentMode::AutoVsync,
            );
            let surface = pollster::block_on(surface_future).expect("Error creating surface");

            renderers.resize_with(context.devices.len(), || None);
            renderers[surface.dev_id].get_or_insert_with(|| {
                Compositor::new(
                    &context.devices[surface.dev_id].device,
                    &context.devices[surface.dev_id].queue,
                )
                .expect("Couldn't create renderer")
            });

            runner.editor.state = RenderState::Active {
                surface: Box::new(surface),
                valid_surface: true,
                window,
            };
        }

        #[cfg(target_arch = "wasm32")]
        {
            let canvas = window.canvas().expect("Winit web canvas");
            runner.editor.state = RenderState::Active { canvas, window };
        }

        if let RenderState::Active { window, .. } = &runner.editor.state {
            let window = window.clone();
            let size = window.inner_size();
            runner.refresh_frame(
                window.scale_factor(),
                Size::new(size.width as f64, size.height as f64),
            );
            runner.sync_cursor(&window);
            window.request_redraw();
        }
    }

    fn editor_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        index: usize,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let runner = &mut self.editors[index];
        let window = match &runner.editor.state {
            RenderState::Active { window, .. } if window.id() == window_id => window.clone(),
            _ => return,
        };
        let scale = window.scale_factor();
        // Preserve motion samples without minting intermediate frames.
        // Discrete input (including release/cancel) first settles the batch.
        if runner.flush_before_window_event(&event) {
            window.request_redraw();
        }

        if let WindowEvent::ModifiersChanged(state) = &event {
            let size = window.inner_size();
            runner.modifiers_changed(
                ui_events_winit::keyboard::from_winit_modifier_state(state.state()),
                scale,
                Size::new(size.width as f64, size.height as f64),
            );
            window.request_redraw();
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
            let translation = translate_window_event(&mut runner.editor.reducer, scale, &event);
            let size = window.inner_size();
            let viewport = Size::new(size.width as f64, size.height as f64);
            let redraw = match (ime, translation) {
                (Some(ime), _) => runner.ime_event(&ime, scale, viewport),
                (None, Some(WindowEventTranslation::Keyboard(event))) => {
                    runner.keyboard_event(&event, scale, viewport)
                }
                (None, Some(WindowEventTranslation::Pointer(event))) => {
                    runner.pointer_event(&event, scale, viewport)
                }
                _ => false,
            };
            if redraw {
                window.request_redraw();
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                #[cfg(any(target_os = "macos", target_os = "linux"))]
                self.request_discard(event_loop, index, AfterDiscard::CloseWindow);
                #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
                self.request_discard(event_loop, index, AfterDiscard::Quit);
            }

            WindowEvent::Resized(size) => {
                let valid = size.width != 0 && size.height != 0;
                #[cfg(not(target_arch = "wasm32"))]
                if let RenderState::Active {
                    surface,
                    valid_surface,
                    ..
                } = &mut self.editors[index].editor.state
                {
                    if valid {
                        self.context
                            .resize_surface(surface, size.width, size.height);
                    }
                    *valid_surface = valid;
                }
                if valid {
                    self.redraw(index);
                }
            }

            WindowEvent::ScaleFactorChanged { .. } => {
                window.request_redraw();
            }

            // The hover is the pointer RELATIVE TO CONTENT, and a
            // moved window shifts that relation with no pointer event
            // — and no way to re-measure it (a title-bar drag carries
            // the mouse along; an OS-driven move doesn't; winit can't
            // say where the pointer now sits). The honest state is
            // unknown until the next move.
            WindowEvent::Moved(_) => {
                let runner = &mut self.editors[index];
                let size = window.inner_size();
                if runner.window_moved(scale, Size::new(size.width as f64, size.height as f64)) {
                    window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => self.redraw(index),
            _ => {}
        }
    }
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static WEB_PROXY: std::cell::RefCell<Option<winit::event_loop::EventLoopProxy<UserEvent>>> =
        const { std::cell::RefCell::new(None) };
}

/// Called by the JS host on the page thread, never on the computation worker.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn computation_finished() {
    WEB_PROXY.with(|proxy| {
        if let Some(proxy) = &*proxy.borrow() {
            let _ = proxy.send_event(UserEvent::ComputationFinished);
        }
    });
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen(js_name = start_editor))]
pub fn run() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let doc_path = std::env::args().nth(1).map(PathBuf::from).map(canonical);
    #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
    let doc_path: Option<PathBuf> = None;
    // A given-but-missing path is a new document there; no path is
    // untitled until the first save asks. A file that exists but does
    // not parse is refused rather than silently replaced, so a save
    // cannot clobber it with the sample.
    let (doc, binders) = match &doc_path {
        Some(path) if path.exists() => text_store::load(path).unwrap_or_else(|error| {
            eprintln!("failed to load {}: {error}", path.display());
            std::process::exit(1);
        }),
        // No path starts EMPTY — the sample lives in examples/sample.gid now,
        // opened like any document.
        _ => (
            gid::Document {
                root: None,
                cells: gid::Cells::new(),
            },
            gid_text::Binders::new(),
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
    #[cfg(target_arch = "wasm32")]
    WEB_PROXY.with(|slot| *slot.borrow_mut() = Some(proxy.clone()));
    #[cfg(target_os = "macos")]
    let native_menu = native_menu::Menu::new(proxy.clone());

    let stack = stack::load();
    let fonts = font_context();
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let drawn_menu = platform::DRAWN_MENU || std::env::var_os("PROGRED_DRAWN_MENU").is_some();
    #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
    let drawn_menu = platform::DRAWN_MENU;
    #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
    let mut app = App {
        #[cfg(target_arch = "wasm32")]
        web_renderer: web_render::take(),
        #[cfg(not(target_arch = "wasm32"))]
        context: RenderContext::new(),
        #[cfg(not(target_arch = "wasm32"))]
        renderers: vec![],
        stack: stack.clone(),
        fonts: fonts.clone(),
        #[cfg(target_os = "macos")]
        native_menu,
        proxy: proxy.clone(),
        drawn_menu,
        focused: None,
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        quit: QuitState::Idle,
        #[cfg(target_os = "macos")]
        cascade: macos_window::initial_cascade(),
        editors: vec![EditorRunner::new(new_editor(
            drawn_menu,
            stack,
            fonts,
            doc,
            doc_path,
            binders,
            Some(proxy),
        ))],
    };

    #[cfg(not(target_arch = "wasm32"))]
    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");
    #[cfg(target_arch = "wasm32")]
    event_loop.spawn_app(app);
}

#[cfg(target_os = "ios")]
#[unsafe(no_mangle)]
pub extern "C" fn progred_start() {
    run();
}

impl Editor {
    fn window_id(&self) -> Option<WindowId> {
        self.window().map(|window| window.id())
    }

    fn window(&self) -> Option<Arc<Window>> {
        match &self.state {
            RenderState::Active { window, .. } => Some(window.clone()),
            RenderState::Suspended(window) => window.clone(),
        }
    }

    fn advance_gesture(&mut self, samples: &[Point]) -> bool {
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

    fn finish_gesture(&mut self) -> bool {
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
        // The macOS convention: the display name alone — the dirty
        // state is the close button's dot, the location the proxy
        // icon.
        #[cfg(target_os = "macos")]
        return match &self.doc_path {
            Some(path) => path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string()),
            None => "Untitled".to_string(),
        };
        #[cfg(not(target_os = "macos"))]
        {
            let dirty = if self.model.dirty() { " •" } else { "" };
            match &self.doc_path {
                Some(path) => format!("Progred — {}{dirty}", path.display()),
                None => format!("Progred — untitled{dirty}"),
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
        command::Toggles {
            raw: self
                .model
                .workspace
                .selected_or_document(
                    self.model
                        .selection
                        .as_ref()
                        .map(selection::Selection::root),
                )
                .projection
                == workspace::Projection::Raw,
            debug_geometry: self.model.view.debug_geometry,
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
        // The native menu owns its own shortcuts; only the drawn
        // menu routes keys here.
        if !self.drawn_menu {
            return false;
        }
        if self.menu.open().is_some() {
            if event.state.is_down()
                && modifiers::plain(&event.modifiers)
                && matches!(event.key, Key::Named(NamedKey::Escape))
            {
                return self.menu.close();
            }
            let availability = self.menu_availability();
            return match menu::navigate(&mut self.menu, &menu::definition(), availability, event) {
                menu::Navigation::Activate(command) => {
                    self.choose_menu(command, geometry);
                    true
                }
                menu::Navigation::Handled => true,
                menu::Navigation::Pass => self.menu.captures_key(event),
            };
        }
        menu::shortcut(event)
            .filter(|command| self.menu_availability().enabled(*command))
            .is_some_and(|command| {
                self.choose_menu(command, geometry);
                true
            })
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
    fn replace_document(
        &mut self,
        doc: gid::Document,
        path: Option<PathBuf>,
        text_binders: gid_text::Binders,
    ) {
        self.finish_gesture();
        // Exhaustive: a new Editor field must explicitly choose its lifetime here.
        let Self {
            computations,
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

impl App {
    /// Menu enablement follows the model: gray what can't act. Save
    /// stays live for untitled documents — it defers to the save
    /// panel, per platform convention.
    pub(crate) fn sync_menus(&self, index: usize) {
        #[cfg(target_os = "macos")]
        {
            let editor = &self.editors[index].editor;
            self.native_menu
                .sync(Some((editor.menu_availability(), editor.menu_toggles())));
        }
        #[cfg(not(target_os = "macos"))]
        let _ = index;
    }

    /// Application commands need no window; document commands act on
    /// the focused window's editor (the native menu's routing — the
    /// drawn menu dispatches document commands to its own editor
    /// directly).
    pub(crate) fn run_command(&mut self, event_loop: &ActiveEventLoop, command: Command) {
        match command {
            Command::App(command) => self.run_app_command(event_loop, command),
            Command::Doc(command) => {
                if let Some(index) = self.focused_index() {
                    let runner = &mut self.editors[index];
                    if let Some(window) = runner.editor.window() {
                        let size = window.inner_size();
                        runner.update_frame(
                            window.scale_factor(),
                            Size::new(size.width as f64, size.height as f64),
                            |editor, dispatch, _| {
                                editor.run_doc_command(
                                    command,
                                    dispatch.geometry(window.scale_factor()),
                                );
                                frame::FrameDisposition::Remint
                            },
                        );
                        window.request_redraw();
                    } else {
                        runner
                            .editor
                            .run_doc_command(command, runner.frame.dispatch.geometry(1.0));
                    }
                }
            }
        }
    }

    /// New and examples are in-place development shortcuts. New Window and
    /// Open retain desktop new-window behavior.
    fn run_app_command(&mut self, event_loop: &ActiveEventLoop, command: AppCommand) {
        match command {
            AppCommand::New => self.new_document(
                event_loop,
                gid::Document {
                    root: None,
                    cells: gid::Cells::new(),
                },
                gid_text::Binders::new(),
            ),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            AppCommand::NewWindow => self.open_editor(
                event_loop,
                gid::Document {
                    root: None,
                    cells: gid::Cells::new(),
                },
                None,
                gid_text::Binders::new(),
            ),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            AppCommand::Open => {
                if let Some(path) = text_dialog().pick_file().map(canonical) {
                    match text_store::load(&path) {
                        Ok((doc, binders)) => {
                            self.open_editor(event_loop, doc, Some(path), binders)
                        }
                        Err(error) => {
                            eprintln!("failed to open {}: {error}", path.display());
                        }
                    }
                }
            }
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            AppCommand::Close => {
                if let Some(index) = self.focused_index() {
                    self.request_discard(event_loop, index, AfterDiscard::CloseWindow);
                }
            }
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            AppCommand::Quit => self.begin_quit(event_loop),
            #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
            AppCommand::Quit => {
                if let Some(index) = self.focused_index() {
                    self.request_discard(event_loop, index, AfterDiscard::Quit);
                }
            }
            AppCommand::Example(example) => match gid_text::parse(example.source()) {
                Ok((doc, binders)) => self.new_document(event_loop, doc, binders),
                Err(error) => panic!("built-in example failed to parse: {error}"),
            },
        }
    }

    fn new_document(
        &mut self,
        event_loop: &ActiveEventLoop,
        doc: gid::Document,
        binders: gid_text::Binders,
    ) {
        if let Some(index) = self.focused_index() {
            self.request_discard(event_loop, index, AfterDiscard::Replace { doc, binders });
        } else {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            self.open_editor(event_loop, doc, None, binders);
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
    pub(crate) fn request_discard(
        &mut self,
        event_loop: &ActiveEventLoop,
        index: usize,
        then: AfterDiscard,
    ) {
        if self.editors[index].editor.pending_discard.is_some() {
            return;
        }
        if !self.editors[index].editor.model.dirty() {
            self.proceed(event_loop, index, then);
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let accepted = web_sys::window()
                .and_then(|window| window.confirm_with_message("Discard unsaved changes?").ok())
                .unwrap_or(false);
            if accepted {
                self.proceed(event_loop, index, then);
            }
            return;
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let editor = &mut self.editors[index].editor;
            let RenderState::Active { window, .. } = &editor.state else {
                return;
            };
            let window_id = window.id();
            editor.pending_discard = Some(then);
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
                let _ = proxy.send_event(UserEvent::Discard {
                    window: window_id,
                    accepted,
                });
            });
        }
        #[cfg(target_os = "ios")]
        self.proceed(event_loop, index, then);
    }

    /// The action a confirmed (or unneeded) discard proceeds to.
    pub(crate) fn proceed(
        &mut self,
        event_loop: &ActiveEventLoop,
        index: usize,
        then: AfterDiscard,
    ) {
        match then {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            AfterDiscard::CloseWindow => {
                self.close_editor(event_loop, index);
                self.advance_quit(event_loop);
            }
            AfterDiscard::Replace { doc, binders } => {
                self.editors[index].adopt_model(doc, None, binders);
                if self.focused_index() == Some(index) {
                    self.sync_menus(index);
                }
            }
            #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
            AfterDiscard::Quit => event_loop.exit(),
        }
    }

    /// Renders the current model to the surface, from `RedrawRequested`.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn redraw(&mut self, index: usize) {
        // The menu bar mirrors the focused window's editor only; an
        // unfocused window's redraw must not relabel it.
        if self.focused_index() == Some(index) {
            self.sync_menus(index);
        }
        let App {
            editors,
            context,
            renderers,
            ..
        } = &mut *self;
        let runner = &mut editors[index];
        let RenderState::Active {
            surface,
            valid_surface: true,
            window,
        } = &runner.editor.state
        else {
            return;
        };
        let window = window.clone();
        let scale = window.scale_factor();
        let width = surface.config.width;
        let height = surface.config.height;

        let viewport = Size::new(width as f64, height as f64);
        let PendingPaint { renders, .. } = runner.prepare_paint(scale, viewport);
        runner.sync_cursor(&window);
        let mut paint = Paint::default();
        puri::frame::render(renders, &mut paint);
        let layers = paint.finish();

        let RenderState::Active { surface, .. } = &mut runner.editor.state else {
            return;
        };
        let device_handle = &context.devices[surface.dev_id];

        let output = renderers[surface.dev_id]
            .as_mut()
            .unwrap()
            .render(
                &device_handle.device,
                &device_handle.queue,
                &layers,
                &mut runner.editor.paint_resources,
                &surface.target_texture,
                Color::new([0.965, 0.965, 0.972, 1.0]),
            )
            .expect("failed to render to texture");

        let surface_texture = match surface.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Suboptimal(_) => {
                context.configure_surface(surface);
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
            &output.texture.create_view(&Default::default()),
            &surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default()),
        );
        device_handle.queue.submit([encoder.finish()]);
        surface_texture.present();

        device_handle.device.poll(wgpu::PollType::Poll).unwrap();
        if runner.frame_presented() {
            window.request_redraw();
        }
    }

    /// Browser presentation uses the same GPU compositor as native when
    /// available. Only setup/presentation differs; widgets remain unchanged.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn redraw(&mut self, index: usize) {
        if self.focused_index() == Some(index) {
            self.sync_menus(index);
        }
        let runner = &mut self.editors[index];
        let RenderState::Active { canvas, window } = &runner.editor.state else {
            return;
        };
        let canvas = canvas.clone();
        let window = window.clone();
        let scale = window.scale_factor();
        let size = window.inner_size();
        let width = size.width;
        let height = size.height;
        if width == 0 || height == 0 {
            return;
        }
        if canvas.width() != width {
            canvas.set_width(width);
        }
        if canvas.height() != height {
            canvas.set_height(height);
        }

        let viewport = Size::new(width as f64, height as f64);
        let PendingPaint { renders, .. } = runner.prepare_paint(scale, viewport);
        runner.sync_cursor(&window);
        let presented = self
            .web_renderer
            .render(
                width,
                height,
                Color::new([0.965, 0.965, 0.972, 1.0]),
                |canvas| puri::frame::render(renders, canvas),
            )
            .expect("browser render failed");
        if !presented || runner.frame_presented() {
            window.request_redraw();
        }
    }
}

#[cfg(test)]
mod shell_tests {
    use super::*;
    use crate::input::{pointer_position, window_pointer};
    use ui_events::pointer::{PointerId, PointerInfo, PointerState, PointerType};
    use winit::dpi::PhysicalPosition;
    use winit::event::{DeviceId, ElementState, MouseButton};

    fn translate_pointer(reducer: &mut WindowEventReducer, event: WindowEvent) -> PointerEvent {
        match translate_window_event(reducer, 1.0, &event) {
            Some(WindowEventTranslation::Pointer(pointer)) => pointer,
            _ => panic!("expected pointer input"),
        }
    }

    #[test]
    fn window_departure_preserves_pressed_motion_and_release_outside() {
        let mut reducer = WindowEventReducer::default();
        let device_id = DeviceId::dummy();
        assert!(matches!(
            translate_pointer(
                &mut reducer,
                WindowEvent::MouseInput {
                    device_id,
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                }
            ),
            PointerEvent::Down(_)
        ));
        assert!(matches!(
            translate_pointer(&mut reducer, WindowEvent::CursorLeft { device_id }),
            PointerEvent::Leave(_)
        ));
        let PointerEvent::Move(motion) = translate_pointer(
            &mut reducer,
            WindowEvent::CursorMoved {
                device_id,
                position: PhysicalPosition::new(-40.0, 700.0),
            },
        ) else {
            panic!("expected motion")
        };
        assert!(puri::interact::is_primary_contact_move(&motion));
        assert_eq!(
            pointer_position(&PointerEvent::Move(motion)),
            Some(Point::new(-40.0, 700.0))
        );
        let PointerEvent::Up(release) = translate_pointer(
            &mut reducer,
            WindowEvent::MouseInput {
                device_id,
                state: ElementState::Released,
                button: MouseButton::Left,
            },
        ) else {
            panic!("expected release")
        };
        assert!(release.state.buttons.is_empty());
        assert_eq!(
            pointer_position(&PointerEvent::Up(release)),
            Some(Point::new(-40.0, 700.0))
        );
    }

    #[test]
    fn focus_loss_cancels_and_forgets_pressed_mouse_state() {
        let mut reducer = WindowEventReducer::default();
        let device_id = DeviceId::dummy();
        translate_pointer(
            &mut reducer,
            WindowEvent::MouseInput {
                device_id,
                state: ElementState::Pressed,
                button: MouseButton::Left,
            },
        );
        assert!(matches!(
            translate_pointer(&mut reducer, WindowEvent::Focused(false)),
            PointerEvent::Cancel(_)
        ));
        let PointerEvent::Move(motion) = translate_pointer(
            &mut reducer,
            WindowEvent::CursorMoved {
                device_id,
                position: PhysicalPosition::new(10.0, 20.0),
            },
        ) else {
            panic!("expected motion")
        };
        assert!(!puri::interact::is_primary_contact_move(&motion));
    }

    #[test]
    fn outside_drag_positions_do_not_become_hover_positions() {
        let size = Size::new(400.0, 300.0);
        assert_eq!(
            window_pointer(Point::new(10.0, 20.0), size),
            Some(Point::new(10.0, 20.0))
        );
        for point in [
            Point::new(-1.0, 20.0),
            Point::new(401.0, 20.0),
            Point::new(10.0, -1.0),
            Point::new(10.0, 301.0),
        ] {
            assert_eq!(window_pointer(point, size), None);
        }
    }

    fn pending(delta: ScrollDelta, x: f64) -> PendingScroll {
        let mut state = PointerState::default();
        state.position.x = x;
        PendingScroll {
            events: vec![PointerScrollEvent {
                pointer: PointerInfo {
                    pointer_id: Some(PointerId::PRIMARY),
                    persistent_device_id: None,
                    pointer_type: PointerType::Mouse,
                },
                delta,
                state,
            }],
            scale: 2.0,
            viewport: Size::new(1800.0, 1280.0),
        }
    }

    #[test]
    fn pending_scrolls_preserve_packets_in_order_including_mixed_units() {
        let mut accumulated = pending(
            ScrollDelta::PixelDelta(PhysicalPosition::new(2.0, 3.0)),
            10.0,
        );
        assert!(
            accumulated
                .merge(pending(
                    ScrollDelta::PixelDelta(PhysicalPosition::new(5.0, 7.0)),
                    20.0,
                ))
                .is_ok()
        );
        assert_eq!(
            accumulated
                .events
                .iter()
                .map(|event| (event.delta, event.state.position.x))
                .collect::<Vec<_>>(),
            [
                (
                    ScrollDelta::PixelDelta(PhysicalPosition::new(2.0, 3.0)),
                    10.0
                ),
                (
                    ScrollDelta::PixelDelta(PhysicalPosition::new(5.0, 7.0)),
                    20.0
                ),
            ]
        );
        assert!(
            accumulated
                .merge(pending(ScrollDelta::LineDelta(0.0, 1.0), 20.0))
                .is_ok()
        );
        assert_eq!(
            accumulated.events.last().unwrap().delta,
            ScrollDelta::LineDelta(0.0, 1.0)
        );
        let mut resized = pending(ScrollDelta::LineDelta(0.0, 1.0), 20.0);
        resized.viewport.width += 1.0;
        assert!(accumulated.merge(resized).is_err());
        let mut rescaled = pending(ScrollDelta::LineDelta(0.0, 1.0), 20.0);
        rescaled.scale += 1.0;
        assert!(accumulated.merge(rescaled).is_err());
    }

    fn pointer(x: f64) -> PendingPointer {
        let scroll = pending(ScrollDelta::LineDelta(0.0, 0.0), x);
        let event = scroll.events.into_iter().next().unwrap();
        PendingPointer {
            event: PointerUpdate {
                pointer: event.pointer,
                current: event.state,
                coalesced: vec![],
                predicted: vec![],
            },
            start: Point::ZERO,
            scale: scroll.scale,
            viewport: scroll.viewport,
        }
    }

    #[test]
    fn pointer_batches_preserve_observed_samples_and_replace_predictions() {
        let mut batch = pointer(2.0);
        batch.event.coalesced.push(pointer(1.0).event.current);
        batch.event.predicted.push(pointer(100.0).event.current);
        let mut next = pointer(4.0);
        next.event.coalesced.push(pointer(3.0).event.current);
        next.event.predicted.push(pointer(5.0).event.current);
        next.start = Point::new(2.0, 0.0);
        assert!(batch.merge(next).is_ok());
        assert_eq!(batch.start, Point::ZERO);
        assert_eq!(batch.event.current.position.x, 4.0);
        assert_eq!(
            puri::interact::pointer_samples(&batch.event)
                .map(|sample| sample.position.x)
                .collect::<Vec<_>>(),
            [1.0, 2.0, 3.0, 4.0],
        );
        assert_eq!(batch.event.predicted, vec![pointer(5.0).event.current]);
    }

    #[test]
    fn pointer_batches_do_not_mix_contacts_buttons_or_coordinate_systems() {
        let mut other_contact = pointer(1.0);
        other_contact.event.pointer.pointer_id = PointerId::new(2);
        let mut pressed = pointer(1.0);
        pressed
            .event
            .current
            .buttons
            .insert(ui_events::pointer::PointerButton::Primary);
        let mut modified = pointer(1.0);
        modified.event.current.modifiers = Modifiers::SHIFT;
        let mut resized = pointer(1.0);
        resized.viewport.width += 1.0;
        let mut rescaled = pointer(1.0);
        rescaled.scale = 1.0;
        for next in [other_contact, pressed, modified, resized, rescaled] {
            let mut batch = pointer(0.0);
            let before = batch.event.clone();
            assert!(batch.merge(next).is_err());
            assert_eq!(batch.event, before);
        }
        let mut pressed = pointer(1.0);
        pressed
            .event
            .current
            .buttons
            .insert(ui_events::pointer::PointerButton::Primary);
        let mut next = pointer(2.0);
        next.event.current.buttons = pressed.event.current.buttons;
        assert!(pressed.merge(next).is_ok(), "pressed motion batches too");
    }

    #[test]
    fn redraw_release_and_cancellation_flush_motion_before_dispatch() {
        let device_id = DeviceId::dummy();
        assert!(continuous_input(&WindowEvent::CursorMoved {
            device_id,
            position: PhysicalPosition::new(-20.0, 30.0),
        }));
        for event in [
            WindowEvent::RedrawRequested,
            WindowEvent::MouseInput {
                device_id,
                state: ElementState::Released,
                button: MouseButton::Left,
            },
            WindowEvent::Focused(false),
            WindowEvent::CursorLeft { device_id },
        ] {
            assert!(!continuous_input(&event));
        }
        for (phase, continuous) in [
            (winit::event::TouchPhase::Started, false),
            (winit::event::TouchPhase::Moved, true),
            (winit::event::TouchPhase::Ended, false),
            (winit::event::TouchPhase::Cancelled, false),
        ] {
            assert_eq!(
                continuous_input(&WindowEvent::Touch(winit::event::Touch {
                    device_id,
                    phase,
                    location: PhysicalPosition::new(10.0, 20.0),
                    force: None,
                    id: 1,
                })),
                continuous
            );
        }
    }

    #[test]
    fn divider_hover_uses_the_cursor_for_its_resize_axis() {
        assert_eq!(cursor_icon(None), CursorIcon::Default);
        assert_eq!(
            cursor_icon(Some(&Hovered::Divider(workspace::Divider::Columns(
                workspace::Side::Left,
            )))),
            CursorIcon::ColResize
        );
        assert_eq!(
            cursor_icon(Some(&Hovered::Divider(workspace::Divider::Panes {
                side: workspace::Side::Left,
                before: workspace::Root::document(),
                after: workspace::Root::document(),
            }))),
            CursorIcon::RowResize
        );
    }
}

#[cfg(test)]
pub(crate) fn test_editor(doc: gid::Document) -> Editor {
    let mut editor = new_editor(
        false,
        stack::load(),
        FontContext::new(),
        doc,
        None,
        Default::default(),
        None,
    );
    editor.model.workspace.document.root = test_root();
    editor
}

#[cfg(test)]
pub(crate) fn test_root() -> workspace::Root {
    thread_local! { static ROOT: workspace::Root = workspace::Root::document(); }
    ROOT.with(Clone::clone)
}
