//! Window shell: winit + Vello plumbing around pure frame drawing.
//! `app_view` renders to any puri `Canvas`; here its deferred ink
//! streams into vello.

mod annotations;
mod command;
mod commands;
mod completion;
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
mod workspace;

use crate::command::{AppCommand, Command, DocCommand};
use crate::frame::{Dispatch, FrameDisposition, Hovered, Paint, frame_disposition};
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
use ui_events::ScrollDelta;
use ui_events::keyboard::{Key, KeyboardEvent, Modifiers, NamedKey};
use ui_events::pointer::{
    PointerEvent, PointerId, PointerInfo, PointerScrollEvent, PointerType, PointerUpdate,
};
use ui_events_winit::{WindowEventReducer, WindowEventTranslation};
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "ios")))]
use vello::util::{RenderContext, RenderSurface};
#[cfg(not(target_arch = "wasm32"))]
use vello::wgpu::{self, CurrentSurfaceTexture};
#[cfg(not(target_arch = "wasm32"))]
use vello::{AaConfig, Renderer, RendererOptions, Scene};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
use winit::application::ApplicationHandler;
#[cfg(not(target_arch = "wasm32"))]
use winit::dpi::LogicalSize;
use winit::dpi::PhysicalPosition;
use winit::event::{Ime, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
#[cfg(target_arch = "wasm32")]
use winit::platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys, WindowExtWebSys};
#[cfg(target_os = "linux")]
use winit::platform::x11::WindowAttributesExtX11;
use winit::window::{CursorIcon, Window, WindowId};

/// Everything arriving through the event-loop proxy.
pub(crate) enum UserEvent {
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
        context: CanvasRenderingContext2d,
        window: Arc<Window>,
    },
    Suspended(Option<Arc<Window>>),
}

/// The pasteboard type structural copies ride under, beside their
/// plain text; its PRESENCE is the structure/text distinction, so
/// text that merely spells Value JSON is never mistaken for a copy.
#[cfg(any(target_os = "macos", target_os = "linux"))]
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
    pub(crate) hovered_secondary: Option<hover::Secondary>,
    pub(crate) hovered_trace: Option<hover::SourceTrace>,
}

struct PendingScroll {
    event: PointerScrollEvent,
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
            | WindowEvent::Touch(winit::event::Touch {
                phase: winit::event::TouchPhase::Moved,
                ..
            })
    )
}

impl PendingScroll {
    fn merge(&mut self, next: Self) -> Result<(), Self> {
        let merged = self.scale == next.scale
            && self.viewport == next.viewport
            && match (&mut self.event.delta, next.event.delta) {
                (ScrollDelta::PageDelta(x, y), ScrollDelta::PageDelta(next_x, next_y))
                | (ScrollDelta::LineDelta(x, y), ScrollDelta::LineDelta(next_x, next_y)) => {
                    *x += next_x;
                    *y += next_y;
                    true
                }
                (ScrollDelta::PixelDelta(delta), ScrollDelta::PixelDelta(next)) => {
                    delta.x += next.x;
                    delta.y += next.y;
                    true
                }
                _ => false,
            };
        if merged {
            self.event.pointer = next.event.pointer;
            self.event.state = next.event.state;
            Ok(())
        } else {
            Err(next)
        }
    }
}

/// Process-wide state: the GPU, the shared caches, and the editors.
pub(crate) struct App {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) context: RenderContext,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) renderers: Vec<Option<Renderer>>,
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
    pub(crate) editors: Vec<Editor>,
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
    /// This window draws its own menu bar (the drawn menu system).
    pub(crate) drawn_menu: bool,
    pub(crate) state: RenderState,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) scene: Scene,
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
    /// Last pointer position, for anchoring pinch zoom.
    pub(crate) cursor: Point,
    cursor_icon: CursorIcon,
    /// The pointer position while it is inside the window. It is an
    /// input to placement's internal hover resolution.
    pub(crate) pointer: Option<Point>,
    /// Derived from pointer input and settled geometry. Kept outside
    /// the model for air hysteresis, pressed-gesture freezing, and the
    /// event-to-redraw handoff.
    pub(crate) hover: Option<Hovered>,
    /// Current platform modifier state, an ordinary frame input.
    pub(crate) modifiers: Modifiers,
    /// A button is down: gestures keep the hover they began with, so
    /// hover resolution stands down until release.
    pub(crate) pressed: bool,
    /// The selection identity last scrolled into view — path AND
    /// variant, since Enter keeps the path while opening a pending —
    /// so reveal fires once per change and never fights manual
    /// scrolling.
    pub(crate) revealed: Option<(workspace::Root, gid::Path, selection::Stage)>,
    /// A minted frame's event surface, retained until an event spends
    /// it. `pending_paint` carries the same successor frame's pixels.
    pub(crate) dispatch: Option<Dispatch>,
    /// Ink from the successor frame already minted after an event.
    /// The next redraw consumes it instead of minting that frame twice.
    pub(crate) pending_paint: Option<PendingPaint>,
    /// Consecutive scroll packets are one continuous displacement.
    /// Hold them until paint or another event establishes an ordering
    /// boundary, then dispatch their sum through the retained frame.
    pending_scroll: Option<PendingScroll>,
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
    /// The continuation installed by the accepting projection handler.
    gesture: Option<gesture::Active<Editor>>,
    pub(crate) reducer: WindowEventReducer,
    /// Routes the discard sheet's answer back into the loop.
    #[cfg_attr(any(target_arch = "wasm32", target_os = "ios"), allow(dead_code))]
    pub(crate) proxy: Option<winit::event_loop::EventLoopProxy<UserEvent>>,
    pub(crate) pending_discard: Option<AfterDiscard>,
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
    Editor {
        drawn_menu,
        state: RenderState::Suspended(None),
        #[cfg(not(target_arch = "wasm32"))]
        scene: Scene::new(),
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
        cursor_icon: CursorIcon::Default,
        pointer: None,
        hover: None,
        modifiers: Modifiers::empty(),
        pressed: false,
        revealed: None,
        dispatch: None,
        pending_paint: None,
        pending_scroll: None,
        pending_pointer: None,
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

fn window_pointer(position: Point, size: Size) -> Option<Point> {
    Rect::from_origin_size(Point::ZERO, size)
        .contains(position)
        .then_some(position)
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
        for editor in &mut self.editors {
            if editor.flush_pending_continuous()
                && let RenderState::Active { window, .. } = &editor.state
            {
                window.request_redraw();
            }
        }
        match event {
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
                let pending = self.editors[index].pending_discard.take();
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
            self.focused = self.editors.first().and_then(Editor::window_id);
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        for editor in &mut self.editors {
            editor.flush_pending_continuous();
            if let RenderState::Active { window, .. } = &editor.state {
                editor.state = RenderState::Suspended(Some(window.clone()));
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
            .position(|editor| editor.window_id() == Some(id))
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
        self.editors.push(new_editor(
            self.drawn_menu,
            self.stack.clone(),
            self.fonts.clone(),
            doc,
            path,
            binders,
            Some(self.proxy.clone()),
        ));
        self.resume_editor(event_loop, self.editors.len() - 1);
    }

    /// Removes one window. The surface and window close on drop. The
    /// last close quits where that is the platform convention, and
    /// always quits while the quit chain is draining.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn close_editor(&mut self, event_loop: &ActiveEventLoop, index: usize) {
        let closed = self.editors.remove(index);
        if closed.window_id().is_some() && closed.window_id() == self.focused {
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
            .all(|editor| editor.pending_discard.is_none())
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
            if self.editors[index].model.dirty() {
                if let RenderState::Active { window, .. } = &self.editors[index].state {
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
        let editor = &mut editors[index];
        let RenderState::Suspended(cached_window) = &mut editor.state else {
            return;
        };

        let window = cached_window.take().unwrap_or_else(|| {
            let attributes = Window::default_attributes().with_title(editor.title());
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
                    editor.doc_path.as_deref(),
                    cascade,
                );
                macos_window::set_represented(&window, editor.doc_path.as_deref());
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
                Renderer::new(
                    &context.devices[surface.dev_id].device,
                    RendererOptions::default(),
                )
                .expect("Couldn't create renderer")
            });

            editor.state = RenderState::Active {
                surface: Box::new(surface),
                valid_surface: true,
                window,
            };
        }

        #[cfg(target_arch = "wasm32")]
        {
            let canvas = window.canvas().expect("Winit web canvas");
            let context = canvas
                .get_context("2d")
                .expect("Canvas2D lookup")
                .expect("Canvas2D context")
                .dyn_into::<CanvasRenderingContext2d>()
                .expect("CanvasRenderingContext2D");
            editor.state = RenderState::Active {
                canvas,
                context,
                window,
            };
        }

        if let RenderState::Active { window, .. } = &editor.state {
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
        let editor = &mut self.editors[index];
        let window = match &editor.state {
            RenderState::Active { window, .. } if window.id() == window_id => window.clone(),
            _ => return,
        };
        let scale = window.scale_factor();
        // Preserve motion samples without minting intermediate frames.
        // Discrete input (including release/cancel) first settles the batch.
        if !continuous_input(&event) && editor.flush_pending_continuous() {
            window.request_redraw();
        }

        if let WindowEvent::ModifiersChanged(state) = &event {
            editor.modifiers = ui_events_winit::keyboard::from_winit_modifier_state(state.state());
            let size = window.inner_size();
            editor.retain_dispatch(
                scale,
                Size::new(size.width as f64, size.height as f64),
                false,
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
            let translation = translate_window_event(&mut editor.reducer, scale, &event);
            let previous_cursor = editor.cursor;
            if let Some(WindowEventTranslation::Pointer(pointer)) = &translation
                && let Some(position) = pointer_position(pointer)
            {
                editor.cursor = position;
            }
            // Touch has no preceding hover motion. Mint once at the
            // contact point before dispatch so the ordinary activate
            // fallback sees exactly the target a mouse click would.
            if let Some(WindowEventTranslation::Pointer(PointerEvent::Down(button))) = &translation
                && button.pointer.pointer_type == PointerType::Touch
            {
                let size = window.inner_size();
                editor.pointer = Some(Point::new(button.state.position.x, button.state.position.y));
                editor.retain_dispatch(
                    scale,
                    Size::new(size.width as f64, size.height as f64),
                    false,
                );
            }
            // Continuous input waits for an ordering boundary below.
            // Discrete events dispatch into the retained frame's
            // single-shot handler and immediately mints its successor
            // when handled or when a frame input changes.
            if let Some(WindowEventTranslation::Pointer(PointerEvent::Scroll(update))) =
                &translation
            {
                let size = window.inner_size();
                let next = PendingScroll {
                    event: update.clone(),
                    scale,
                    viewport: Size::new(size.width as f64, size.height as f64),
                };
                if let Some(pending) = editor.queue_scroll(next) {
                    editor.dispatch_scroll_batch(pending);
                }
                window.request_redraw();
            } else if let Some(WindowEventTranslation::Pointer(PointerEvent::Move(update))) =
                &translation
            {
                let size = window.inner_size();
                let position = Point::new(update.current.position.x, update.current.position.y);
                let next = PendingPointer {
                    event: update.clone(),
                    start: previous_cursor,
                    scale,
                    viewport: Size::new(size.width as f64, size.height as f64),
                };
                if let Some(pending) = editor.queue_pointer(next) {
                    editor.dispatch_pointer_batch(&pending);
                }
                editor.cursor = position;
                editor.pointer =
                    window_pointer(position, Size::new(size.width as f64, size.height as f64));
                editor.modifiers = update.current.modifiers;
                window.request_redraw();
            } else if (ime.is_some() || translation.is_some())
                && let Some(dispatch) = editor.dispatch.take()
            {
                let size = window.inner_size();
                let viewport = size.height as f64;
                let mut frame_input_changed = false;
                let handled = match (ime, translation) {
                    (Some(ime), _) => dispatch.handler.dispatch_ime(editor, &ime),
                    // An open Linux menu owns the keyboard; otherwise
                    // its shared shortcuts are application commands.
                    // Remaining keys reach the editor first, except
                    // Cmd+V of STRUCTURE while a pending is open — the
                    // query must never eat Value JSON — then fall
                    // through to the structural commands.
                    (None, Some(WindowEventTranslation::Keyboard(key_event))) => {
                        let mut input = placed::DispatchContext::new(None, None);
                        input.descends = dispatch.descends.clone();
                        editor.menu_key(&key_event)
                            || editor.pending_paste_key(&key_event)
                            || dispatch
                                .handler
                                .dispatch_key_with(editor, &key_event, &mut input)
                            || editor.clipboard_key(&dispatch.descends, &key_event)
                            || editor.delete_key(&dispatch.descends, &key_event)
                            || editor.insert_key(&dispatch.descends, &key_event)
                            || editor.collapse_key(&key_event)
                            || match navigate::step_selection(
                                &dispatch.descends,
                                Some(
                                    editor
                                        .model
                                        .selection
                                        .as_ref()
                                        .map(selection::Selection::root)
                                        .unwrap_or_else(|| editor.model.workspace.document_root()),
                                ),
                                editor.model.selection.as_ref(),
                                dispatch.line,
                                &key_event,
                            ) {
                                Some(target) => {
                                    let select = target.select.clone();
                                    select(editor, navigate::direction(&key_event));
                                    true
                                }
                                None => false,
                            }
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Down(button)))) => {
                        let position = Point::new(button.state.position.x, button.state.position.y);
                        editor.pointer =
                            window_pointer(position, Size::new(size.width as f64, viewport));
                        editor.pressed = true;
                        editor.finish_gesture();
                        frame_input_changed = true;
                        let mut pointer = placed::DispatchContext::new(
                            dispatch.pointer_root.clone(),
                            editor.hover.clone(),
                        );
                        pointer.descends = dispatch.descends.clone();
                        let handled = dispatch.handler.dispatch_pointer_down_with(
                            editor,
                            &button,
                            &mut pointer,
                        );
                        handled
                            || (puri::interact::is_primary_contact(&button)
                                && pointer.hovered.is_none()
                                && editor.model.selection.take().is_some())
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Up(button)))) => {
                        let position = Point::new(button.state.position.x, button.state.position.y);
                        editor.pointer =
                            window_pointer(position, Size::new(size.width as f64, viewport));
                        editor.pressed = false;
                        frame_input_changed = true;
                        editor.finish_gesture()
                            || dispatch.handler.dispatch_pointer_up(editor, &button)
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Leave(_)))) => {
                        editor.pointer = None;
                        frame_input_changed = true;
                        false
                    }
                    (
                        None,
                        Some(WindowEventTranslation::Pointer(PointerEvent::Cancel(pointer))),
                    ) => {
                        editor.pointer = None;
                        editor.pressed = false;
                        frame_input_changed = true;
                        let handled = dispatch.handler.dispatch_pointer_cancel(editor, &pointer);
                        let resize_cancelled = editor.model.workspace.cancel_resize();
                        let gesture_cancelled = editor.finish_gesture();
                        handled || resize_cancelled || gesture_cancelled
                    }
                    _ => false,
                };
                match frame_disposition(handled, frame_input_changed) {
                    FrameDisposition::Retain => editor.dispatch = Some(dispatch),
                    FrameDisposition::Remint { reveal_selection } => {
                        editor.retain_dispatch(
                            scale,
                            Size::new(size.width as f64, viewport),
                            reveal_selection,
                        );
                        window.request_redraw();
                    }
                }
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
                } = &mut self.editors[index].state
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
                let editor = &mut self.editors[index];
                let changed = editor.pointer.take().is_some() || editor.hover.is_some();
                let window = match &editor.state {
                    RenderState::Active { window, .. } => Some(window.clone()),
                    _ => None,
                };
                if changed && let Some(window) = window {
                    let size = window.inner_size();
                    editor.retain_dispatch(
                        window.scale_factor(),
                        Size::new(size.width as f64, size.height as f64),
                        false,
                    );
                    window.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => self.redraw(index),
            _ => {}
        }
    }
}

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
        editors: vec![new_editor(
            drawn_menu,
            stack,
            fonts,
            doc,
            doc_path,
            binders,
            Some(proxy),
        )],
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

    fn queue_scroll(&mut self, next: PendingScroll) -> Option<PendingScroll> {
        match self.pending_scroll.take() {
            None => {
                self.pending_scroll = Some(next);
                None
            }
            Some(mut pending) => match pending.merge(next) {
                Ok(()) => {
                    self.pending_scroll = Some(pending);
                    None
                }
                Err(next) => {
                    self.pending_scroll = Some(next);
                    Some(pending)
                }
            },
        }
    }

    fn queue_pointer(&mut self, next: PendingPointer) -> Option<PendingPointer> {
        match self.pending_pointer.take() {
            None => {
                self.pending_pointer = Some(next);
                None
            }
            Some(mut pending) => match pending.merge(next) {
                Ok(()) => {
                    self.pending_pointer = Some(pending);
                    None
                }
                Err(next) => {
                    self.pending_pointer = Some(next);
                    Some(pending)
                }
            },
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

    fn dispatch_scroll_batch(&mut self, pending: PendingScroll) -> bool {
        let initialized = self.dispatch.is_none();
        if initialized {
            self.retain_dispatch(pending.scale, pending.viewport, false);
        }
        match self.dispatch.take() {
            Some(dispatch) => {
                let outcome = dispatch.handler.dispatch_scroll(self, &pending.event);
                if outcome.handled() {
                    self.retain_dispatch(pending.scale, pending.viewport, true);
                    true
                } else {
                    self.dispatch = Some(dispatch);
                    initialized
                }
            }
            None => false,
        }
    }

    fn flush_pending_scroll(&mut self) -> bool {
        match self.pending_scroll.take() {
            Some(pending) => self.dispatch_scroll_batch(pending),
            None => false,
        }
    }

    fn dispatch_pointer_batch(&mut self, pending: &PendingPointer) -> bool {
        self.cursor = Point::new(
            pending.event.current.position.x,
            pending.event.current.position.y,
        );
        self.pointer = window_pointer(self.cursor, pending.viewport);
        self.modifiers = pending.event.current.modifiers;
        if self.dispatch.is_none() {
            self.retain_dispatch(pending.scale, pending.viewport, false);
        }
        let Some(dispatch) = self.dispatch.take() else {
            return false;
        };
        let event = &pending.event;
        let samples: Vec<_> = puri::interact::pointer_samples(event)
            .map(|sample| Point::new(sample.position.x, sample.position.y))
            .collect();
        let moved =
            self.advance_gesture(&samples) || dispatch.handler.dispatch_pointer_move(self, event);
        // Unclaimed touch motion scrolls through the same nested handlers.
        let handled = moved
            || (event.pointer.pointer_type == PointerType::Touch
                && dispatch
                    .handler
                    .dispatch_scroll(
                        self,
                        &PointerScrollEvent {
                            pointer: event.pointer,
                            delta: ScrollDelta::PixelDelta(PhysicalPosition::new(
                                event.current.position.x - pending.start.x,
                                event.current.position.y - pending.start.y,
                            )),
                            state: event.current.clone(),
                        },
                    )
                    .handled());
        if handled {
            self.retain_dispatch(pending.scale, pending.viewport, true);
            true
        } else {
            self.dispatch = Some(dispatch);
            false
        }
    }

    /// Settle continuous inputs into the frame state. Scroll runs
    /// before the latest pointer sample: scrolling moves content, then
    /// pointer motion observes its final position. If neither handler
    /// spends the frame, pointer movement still requires one remint so
    /// hover sees the new frame input.
    fn flush_pending_continuous(&mut self) -> bool {
        let pointer = self.pending_pointer.take();
        let mut reminted = self.flush_pending_scroll();
        if let Some(pointer) = pointer {
            reminted |= self.dispatch_pointer_batch(&pointer);
            if !reminted {
                self.retain_dispatch(pointer.scale, pointer.viewport, false);
                reminted = true;
            }
        }
        reminted
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

    fn sync_cursor(&mut self, window: &Window) {
        let next = cursor_icon(self.hover.as_ref());
        if next != self.cursor_icon {
            window.set_cursor(next);
            self.cursor_icon = next;
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
                    .and_then(|selection| self.sources().resolve_path(selection.path()))
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
            let value = self.sources().resolve_path(selection.path())?.clone();
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

    pub(crate) fn choose_menu(&mut self, command: Command) {
        self.menu.close();
        match command {
            Command::Doc(command) => self.run_doc_command(command),
            Command::App(_) => {
                if let Some(proxy) = &self.proxy {
                    let _ = proxy.send_event(UserEvent::Command(command));
                }
            }
        }
    }

    /// A document command against this editor, including the redraw
    /// its view changes require.
    pub(crate) fn run_doc_command(&mut self, command: DocCommand) {
        match command {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            DocCommand::Save => self.menu_save(false),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            DocCommand::SaveAs => self.menu_save(true),
            DocCommand::Undo => self.step_history(true),
            DocCommand::Redo => self.step_history(false),
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

    pub(crate) fn menu_key(&mut self, event: &KeyboardEvent) -> bool {
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
                    self.choose_menu(command);
                    true
                }
                menu::Navigation::Handled => true,
                menu::Navigation::Pass => self.menu.captures_key(event),
            };
        }
        menu::shortcut(event)
            .filter(|command| self.menu_availability().enabled(*command))
            .is_some_and(|command| {
                self.choose_menu(command);
                true
            })
    }

    /// Undo or redo one step, restoring the snapshot's document and
    /// selection; the displaced state crosses to the other stack.
    pub(crate) fn step_history(&mut self, back: bool) {
        self.finish_gesture();
        if self.model.step_history(back) {
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
    /// pointer position and held modifiers; old gestures and handlers do not survive.
    pub(crate) fn adopt_model(
        &mut self,
        doc: gid::Document,
        path: Option<PathBuf>,
        text_binders: gid_text::Binders,
    ) {
        self.finish_gesture();
        // Exhaustive: a new Editor field must explicitly choose its lifetime here.
        let Self {
            drawn_menu: _,
            state: _,
            #[cfg(not(target_arch = "wasm32"))]
            scene,
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
            cursor_icon: _,
            pointer: _,
            hover,
            modifiers: _,
            pressed,
            revealed,
            dispatch,
            pending_paint,
            pending_scroll,
            pending_pointer,
            gesture: _,
            reducer: _,
            proxy: _,
            pending_discard,
        } = self;
        #[cfg(target_os = "macos")]
        let changed_path = *doc_path != path;
        *dispatch = None;
        *pending_paint = None;
        *pending_scroll = None;
        *pending_pointer = None;
        *pending_discard = None;
        *pressed = false;
        *hover = None;
        *revealed = None;
        *menu = menu::State::default();
        *binders = text_binders;
        *doc_path = path;
        model.replace_document(doc);
        #[cfg(not(target_arch = "wasm32"))]
        scene.reset();
        self.refresh_title();
        #[cfg(target_os = "macos")]
        if changed_path && let Some(window) = self.window() {
            match self.doc_path.as_deref() {
                Some(path) => macos_window::rename_document_frame(&window, path),
                None => macos_window::clear_document_frame(&window),
            }
        }
        if let RenderState::Active { window, .. } = &self.state {
            let window = window.clone();
            let size = window.inner_size();
            self.retain_dispatch(
                window.scale_factor(),
                Size::new(size.width as f64, size.height as f64),
                false,
            );
            self.sync_cursor(&window);
            window.request_redraw();
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
            let editor = &self.editors[index];
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
                    let editor = &mut self.editors[index];
                    editor.run_doc_command(command);
                    // The native path's frame scheduling, mirroring
                    // the drawn dispatch's handled-event remint.
                    if let RenderState::Active { window, .. } = &editor.state {
                        let window = window.clone();
                        let size = window.inner_size();
                        editor.retain_dispatch(
                            window.scale_factor(),
                            Size::new(size.width as f64, size.height as f64),
                            true,
                        );
                        window.request_redraw();
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
        if self.editors[index].pending_discard.is_some() {
            return;
        }
        if !self.editors[index].model.dirty() {
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
            let editor = &mut self.editors[index];
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
        let editor = &mut editors[index];
        let RenderState::Active {
            surface,
            valid_surface: true,
            window,
        } = &editor.state
        else {
            return;
        };
        let window = window.clone();
        let scale = window.scale_factor();
        let width = surface.config.width;
        let height = surface.config.height;

        let viewport = Size::new(width as f64, height as f64);
        editor.scene.reset();
        let PendingPaint {
            renders,
            hovered_secondary,
            hovered_trace,
            ..
        } = editor.prepare_paint(scale, viewport);
        editor.sync_cursor(&window);
        let ink = placed::Ink {
            hovered: editor.hover.as_ref(),
            hovered_secondary: hovered_secondary.as_ref(),
            hovered_trace: hovered_trace.as_ref(),
            debug_geometry: editor.model.view.debug_geometry,
        };
        let mut paint = Paint {
            scene: std::mem::replace(&mut editor.scene, Scene::new()),
        };
        for render in renders {
            render(&mut paint, ink);
        }
        editor.scene = paint.scene;

        let RenderState::Active { surface, .. } = &mut editor.state else {
            return;
        };
        let device_handle = &context.devices[surface.dev_id];

        renderers[surface.dev_id]
            .as_mut()
            .unwrap()
            .render_to_texture(
                &device_handle.device,
                &device_handle.queue,
                &editor.scene,
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
            &surface.target_view,
            &surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default()),
        );
        device_handle.queue.submit([encoder.finish()]);
        surface_texture.present();

        device_handle.device.poll(wgpu::PollType::Poll).unwrap();
    }

    /// The browser runs the same deferred frame ink directly into
    /// Canvas2D. Layout and event dispatch are shared with desktop;
    /// only this final interpreter differs.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn redraw(&mut self, index: usize) {
        if self.focused_index() == Some(index) {
            self.sync_menus(index);
        }
        let editor = &mut self.editors[index];
        let RenderState::Active {
            canvas,
            context,
            window,
        } = &editor.state
        else {
            return;
        };
        let canvas = canvas.clone();
        let context = context.clone();
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
        let PendingPaint {
            renders,
            hovered_secondary,
            hovered_trace,
            ..
        } = editor.prepare_paint(scale, viewport);
        editor.sync_cursor(&window);
        let ink = placed::Ink {
            hovered: editor.hover.as_ref(),
            hovered_secondary: hovered_secondary.as_ref(),
            hovered_trace: hovered_trace.as_ref(),
            debug_geometry: editor.model.view.debug_geometry,
        };
        let mut paint = Paint {
            canvas: puri_web::WebCanvas(context),
        };
        paint.canvas.clear(
            width.into(),
            height.into(),
            Color::new([0.965, 0.965, 0.972, 1.0]),
        );
        for render in renders {
            render(&mut paint, ink);
        }
    }
}

#[cfg(test)]
mod shell_tests {
    use super::*;
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
            event: PointerScrollEvent {
                pointer: PointerInfo {
                    pointer_id: Some(PointerId::PRIMARY),
                    persistent_device_id: None,
                    pointer_type: PointerType::Mouse,
                },
                delta,
                state,
            },
            scale: 2.0,
            viewport: Size::new(1800.0, 1280.0),
        }
    }

    #[test]
    fn pending_scrolls_sum_same_kind_packets_and_keep_the_latest_state() {
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
            accumulated.event.delta,
            ScrollDelta::PixelDelta(PhysicalPosition::new(7.0, 10.0))
        );
        assert_eq!(accumulated.event.state.position.x, 20.0);
        assert!(
            accumulated
                .merge(pending(ScrollDelta::LineDelta(0.0, 1.0), 20.0))
                .is_err()
        );
    }

    fn pointer(x: f64) -> PendingPointer {
        let scroll = pending(ScrollDelta::LineDelta(0.0, 0.0), x);
        PendingPointer {
            event: PointerUpdate {
                pointer: scroll.event.pointer,
                current: scroll.event.state,
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
