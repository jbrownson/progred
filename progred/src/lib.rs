//! Window shell: winit + Vello plumbing around pure frame drawing.
//! `app_view` renders to any puri `Canvas`; here its deferred ink
//! streams into vello.

mod annotations;
mod command;
mod commands;
mod completion;
mod filter;
mod frame;
mod gid_text;
#[cfg(target_os = "ios")]
mod gpu;
#[cfg(test)]
mod grap_examples;
mod history;
mod hover;
mod identity;
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
use crate::frame::{Dispatch, Frame, FrameDisposition, Hovered, Paint, frame_disposition};
use crate::model::{Model, ViewFlags};
use kurbo::{Point, Rect, Size};
use peniko::{Brush, Color};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(target_os = "ios")]
use gpu::{RenderContext, RenderSurface};
use parley::{FontContext, LayoutContext};
use puri::edit::TextClipboard;
use puri::handler::ImeEvent;
use ui_events::ScrollDelta;
use ui_events::keyboard::{Key, KeyboardEvent, Modifiers, NamedKey};
use ui_events::pointer::{PointerEvent, PointerScrollEvent, PointerType, PointerUpdate};
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
    #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
    New,
    #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
    Quit,
    #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
    Example(command::Example),
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

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) struct SystemTextClipboard;

#[cfg(any(target_arch = "wasm32", target_os = "ios"))]
#[derive(Default)]
pub(crate) struct SystemTextClipboard {
    text: Option<String>,
    structure: Option<gid::Value>,
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
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

#[cfg(any(target_arch = "wasm32", target_os = "ios"))]
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
    pub(crate) renders: Vec<placed::Render<Paint>>,
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
    scale: f64,
    viewport: Size,
}

// Logical pixels. Winit does not expose the platform drag threshold;
// replace this fallback when the input adapter can provide one.
const POINTER_DRAG_SLOP: f64 = 3.0;

struct PendingScrub {
    origin: Point,
    point: Point,
    scale: f64,
    action: placed::ScrubAction,
    gesture: progred_display::ScrubGesture,
    dragging: bool,
    recorded: bool,
    spelling: Option<String>,
}

struct PendingStateDrag {
    origin: Point,
    scale: f64,
    root: workspace::Root,
    path: gid::Path,
    gesture: progred_display::StateDragGesture,
    dragging: bool,
}

struct PendingPoint {
    root: workspace::Root,
    path: gid::Path,
    rect: Rect,
    handler: progred_display::PointHandler,
    recorded: bool,
}

impl PendingPoint {
    fn update(&self, point: Point) -> progred_display::PointUpdate {
        (self.handler)(progred_display::PointEvent {
            x: ((point.x - self.rect.x0) / self.rect.width()).clamp(0.0, 1.0),
            y: ((point.y - self.rect.y0) / self.rect.height()).clamp(0.0, 1.0),
        })
    }
}

impl PendingScrub {
    fn new(origin: Point, scale: f64, action: placed::ScrubAction) -> Self {
        let gesture = (action.handler)();
        Self {
            origin,
            point: origin,
            scale,
            action,
            gesture,
            dragging: false,
            recorded: false,
            spelling: None,
        }
    }

    fn update(&mut self, point: Point) -> Option<progred_display::ScrubEvent> {
        let movement = (point - self.point) / self.scale;
        let distance = (point - self.origin) / self.scale;
        let was_dragging = self.dragging;
        self.dragging |= distance.hypot() >= POINTER_DRAG_SLOP;
        self.point = point;
        self.dragging.then_some(progred_display::ScrubEvent {
            movement_x: if was_dragging { movement.x } else { distance.x },
            distance_y: distance.y,
        })
    }
}

impl PendingStateDrag {
    fn new(
        origin: Point,
        scale: f64,
        root: workspace::Root,
        path: gid::Path,
        handler: progred_display::StateDragHandler,
    ) -> Self {
        let gesture = handler();
        Self {
            origin,
            scale,
            root,
            path,
            gesture,
            dragging: false,
        }
    }

    fn update(&mut self, point: Point) -> Option<progred_display::StateDragEvent> {
        let distance = (point - self.origin) / self.scale;
        self.dragging |= distance.hypot() >= POINTER_DRAG_SLOP;
        self.dragging.then_some(progred_display::StateDragEvent {
            delta_x: distance.x,
            delta_y: distance.y,
        })
    }
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
    pub(crate) drawing_memos: HashMap<workspace::Root, projection::DrawingMemo>,
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
    /// Unpressed pointer motion is continuous frame input, like
    /// scrolling. Keep only its latest sample until paint or a
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
    /// Pressed motion is never deferred; drag gestures receive every
    /// update delivered by the event source.
    pending_pointer: Option<PendingPointer>,
    /// A semantic value scrub retained from its start frame. Raw
    /// pointer input remains available to controls; this runs only as
    /// the editor fallback after the movement threshold is crossed.
    scrub: Option<PendingScrub>,
    /// A host-recognized drag whose result is projection-local view
    /// state rather than a document edit.
    state_drag: Option<PendingStateDrag>,
    /// A continuous point control owns pointer motion until release.
    point: Option<PendingPoint>,
    /// Geometry from the last minted frame, so projection key
    /// handlers can land a delete the same way the shell fallback
    /// does.
    pub(crate) last_descends: Vec<navigate::Descend<Editor>>,
    pub(crate) reducer: WindowEventReducer,
    /// Routes the discard sheet's answer back into the loop.
    #[cfg_attr(any(target_arch = "wasm32", target_os = "ios"), allow(dead_code))]
    pub(crate) proxy: winit::event_loop::EventLoopProxy<UserEvent>,
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
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
) -> Editor {
    Editor {
        drawn_menu,
        state: RenderState::Suspended(None),
        #[cfg(not(target_arch = "wasm32"))]
        scene: Scene::new(),
        font_cx,
        layout_cx: LayoutContext::new(),
        text_cache: puri::text::TextCache::default(),
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        text_clipboard: SystemTextClipboard,
        #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
        text_clipboard: SystemTextClipboard::default(),
        drawing_memos: HashMap::new(),
        stack,
        model: Model {
            doc,
            selection: None,
            history: history::History::default(),
            view: ViewFlags::default(),
            workspace: workspace::Workspace::default(),
        },
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
        scrub: None,
        state_drag: None,
        point: None,
        last_descends: Vec::new(),
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

fn cursor_icon(hover: Option<&Hovered>) -> CursorIcon {
    match hover {
        Some(Hovered::Divider(workspace::Divider::Columns(_))) => CursorIcon::ColResize,
        Some(Hovered::Divider(workspace::Divider::Panes { .. })) => CursorIcon::RowResize,
        _ => CursorIcon::Default,
    }
}

/// The selection as a restorable edge path — pendings restore as
/// nothing, being disposable.
pub(crate) fn edge_path(selection: &Option<selection::Selection>) -> Option<gid::Path> {
    match selection {
        Some(current) if current.stage() == selection::Stage::Edge => Some(current.path().to_vec()),
        _ => None,
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
            self.proxy.clone(),
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
        self.quit = QuitState::Draining { awaiting: None };
        self.advance_quit(event_loop);
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
            if self.editors[index].model.history.dirty() {
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
        // Coalesce by interaction semantics, not by comparing
        // coordinates or trying to recognize Winit's macOS-generated
        // refresh. Real unpressed motion is sampled at frame rate too;
        // the newest position is the frame input. A future consumer
        // that needs the intervening samples can receive them through
        // `PointerUpdate::coalesced` without forcing intermediate
        // projection/layout passes.
        let continuous_pointer =
            matches!(&event, WindowEvent::CursorMoved { .. }) && !editor.pressed;
        if !matches!(&event, WindowEvent::MouseWheel { .. })
            && !continuous_pointer
            && editor.flush_pending_continuous()
        {
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
            let translation = editor.reducer.reduce(scale, &event);
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
            // Scroll packets wait for an ordering boundary below.
            // Every other event dispatches into the retained frame's
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
                && !editor.pressed
            {
                let size = window.inner_size();
                let position = Point::new(update.current.position.x, update.current.position.y);
                editor.pointer = Some(position);
                editor.modifiers = update.current.modifiers;
                editor.pending_pointer = Some(PendingPointer {
                    event: update.clone(),
                    scale,
                    viewport: Size::new(size.width as f64, size.height as f64),
                });
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
                        editor.menu_key(&key_event)
                            || editor.pending_paste_key(&key_event)
                            || dispatch.handler.dispatch_key(editor, &key_event)
                            || editor.clipboard_key(&dispatch.descends, &key_event)
                            || editor.delete_key(&dispatch.descends, &key_event)
                            || editor.insert_key(
                                &dispatch.descends,
                                &dispatch.completion,
                                &key_event,
                            )
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
                                    select(editor);
                                    if let Some(selection) = &mut editor.model.selection {
                                        selection::seed_from_arrow(selection, &key_event);
                                    }
                                    true
                                }
                                None => false,
                            }
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Down(button)))) => {
                        let position = Point::new(button.state.position.x, button.state.position.y);
                        editor.pointer = Some(position);
                        editor.pressed = true;
                        editor.state_drag = None;
                        frame_input_changed = true;
                        let mut pointer = placed::PointerContext::new(
                            dispatch.pointer_root.clone(),
                            editor.hover.clone(),
                        );
                        let pick = modifiers::pick(&button.state.modifiers);
                        let scrub = (pick && modifiers::scrub(&button.state.modifiers))
                            .then(|| {
                                pointer.hovered.as_ref().and_then(|target| {
                                    placed::scrub_target(
                                        &dispatch.scrubs,
                                        pointer.root.as_ref(),
                                        target,
                                    )
                                })
                            })
                            .flatten()
                            .filter(|_| {
                                !matches!(
                                    editor
                                        .model
                                        .selection
                                        .as_ref()
                                        .map(selection::Selection::stage),
                                    Some(selection::Stage::Pending | selection::Stage::Label)
                                )
                            });
                        let handled = dispatch.handler.dispatch_pointer_down_with(
                            editor,
                            &button,
                            &mut pointer,
                        );
                        if pointer.targeted
                            && pick
                            && let Some(Hovered::Tree(hover::Hover::Drawing(source))) =
                                &pointer.hovered
                        {
                            editor.select_drawing_source(&dispatch.descends, source);
                        }
                        if pointer.targeted
                            && let Some(scrub) = scrub
                        {
                            editor.scrub = Some(PendingScrub::new(position, scale, scrub));
                        }
                        handled
                            || (puri::interact::is_primary_contact(&button)
                                && pointer.hovered.is_none()
                                && editor.model.selection.take().is_some())
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Move(update)))) => {
                        // Pointer position is frame input, whether or
                        // not an event handler consumes the motion.
                        let position =
                            Point::new(update.current.position.x, update.current.position.y);
                        editor.pointer = Some(position);
                        frame_input_changed = true;
                        let moved = editor.dispatch_point_move(&update)
                            || dispatch.handler.dispatch_pointer_move(editor, &update)
                            || editor.dispatch_state_drag_move(&update)
                            || editor.dispatch_scrub_move(&update);
                        if moved || update.pointer.pointer_type != PointerType::Touch {
                            moved
                        } else {
                            // A browser canvas has no wheel gesture on
                            // touch. An unclaimed finger drag is the same
                            // continuous displacement sent through the
                            // existing nested scroll handlers; controls
                            // with a raw drag handler still win first.
                            let scroll = PointerScrollEvent {
                                pointer: update.pointer,
                                delta: ScrollDelta::PixelDelta(PhysicalPosition::new(
                                    position.x - previous_cursor.x,
                                    position.y - previous_cursor.y,
                                )),
                                state: update.current.clone(),
                            };
                            dispatch.handler.dispatch_scroll(editor, &scroll).handled()
                        }
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Up(button)))) => {
                        let position = Point::new(button.state.position.x, button.state.position.y);
                        editor.pointer = Some(position);
                        editor.pressed = false;
                        frame_input_changed = true;
                        let handled = dispatch.handler.dispatch_pointer_up(editor, &button);
                        handled
                            || editor.point.take().is_some()
                            || editor.state_drag.take().is_some()
                            || editor.scrub.take().is_some()
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Leave(_)))) => {
                        editor.pointer = None;
                        editor.pressed = false;
                        editor.point = None;
                        editor.state_drag = None;
                        editor.scrub = None;
                        frame_input_changed = true;
                        editor.model.workspace.cancel_resize()
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
                        let point_cancelled = editor.point.take().is_some();
                        let state_drag_cancelled = editor.state_drag.take().is_some();
                        let scrub_cancelled = editor.scrub.take().is_some();
                        handled
                            || resize_cancelled
                            || point_cancelled
                            || state_drag_cancelled
                            || scrub_cancelled
                    }
                    _ => false,
                };
                if handled {
                    editor.finish_handled_event();
                }
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
            drawn_menu, stack, fonts, doc, doc_path, binders, proxy,
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

    fn finish_handled_event(&mut self) {
        let libraries = &self.stack.libraries;
        let model = &mut self.model;
        if let Some(selection) = &mut model.selection {
            let before = model.doc.clone();
            if selection::write_through(&mut model.doc, libraries, selection) {
                let path = selection.path().to_vec();
                model.history.record(before, Some(path));
                self.refresh_title();
            }
        }
    }

    fn dispatch_scrub_move(&mut self, update: &PointerUpdate) -> bool {
        let point = Point::new(update.current.position.x, update.current.position.y);
        let Some(scrub) = &mut self.scrub else {
            return false;
        };
        let Some(event) = scrub.update(point) else {
            return true;
        };
        let path = scrub.action.path.clone();
        let update = (scrub.gesture)(event);
        scrub.spelling = update.spelling;
        let replacement = update.value;
        if self.sources().resolve_path(&path) == Some(&replacement) {
            return true;
        }
        let before = self.model.doc.clone();
        if selection::set_value(
            &mut self.model.doc,
            &self.stack.libraries,
            &path,
            replacement,
        ) {
            if self.scrub.as_ref().is_some_and(|scrub| !scrub.recorded) {
                self.model.history.record(before, Some(path));
                if let Some(scrub) = &mut self.scrub {
                    scrub.recorded = true;
                }
            }
            self.refresh_title();
        }
        true
    }

    fn dispatch_state_drag_move(&mut self, update: &PointerUpdate) -> bool {
        let point = Point::new(update.current.position.x, update.current.position.y);
        let Some(active) = &mut self.state_drag else {
            return false;
        };
        let Some(event) = active.update(point) else {
            return true;
        };
        let root = active.root.clone();
        let path = active.path.clone();
        let state = (active.gesture)(event);
        if let Some(view) = self.model.workspace.view_mut(&root)
            && view.annotations.at(&path) != Some(&state)
        {
            view.annotations.set(&path, Some(state));
        }
        true
    }

    fn start_point(
        &mut self,
        root: workspace::Root,
        path: gid::Path,
        placement: puri::Placement,
        handler: progred_display::PointHandler,
        point: Point,
    ) -> bool {
        self.point = Some(PendingPoint {
            root,
            path,
            rect: placement.rect,
            handler,
            recorded: false,
        });
        self.update_point(point)
    }

    fn dispatch_point_move(&mut self, update: &PointerUpdate) -> bool {
        self.point.is_some()
            && self.update_point(Point::new(
                update.current.position.x,
                update.current.position.y,
            ))
    }

    fn update_point(&mut self, point: Point) -> bool {
        let Some(active) = &self.point else {
            return false;
        };
        let root = active.root.clone();
        let path = active.path.clone();
        let update = active.update(point);
        if self.sources().resolve_path(&path) != Some(&update.value) {
            let before = self.model.doc.clone();
            if selection::set_value(
                &mut self.model.doc,
                &self.stack.libraries,
                &path,
                update.value,
            ) {
                if self.point.as_ref().is_some_and(|point| !point.recorded) {
                    self.model.history.record(before, Some(path.clone()));
                    if let Some(point) = &mut self.point {
                        point.recorded = true;
                    }
                }
                self.refresh_title();
            }
        }
        if let Some(payload) = update.selection {
            let recorded = self
                .model
                .selection
                .as_ref()
                .filter(|selection| selection.root() == &root && selection.path() == path)
                .map(selection::Selection::recorded);
            if let Some(recorded) = recorded {
                let mut next = selection::Selection::from_payload(&self.sources(), path, payload)
                    .with_root(root);
                next.preserve_recorded(recorded);
                self.model.selection = Some(next);
            }
        }
        true
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
                    self.finish_handled_event();
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
        if self.dispatch.is_none() {
            self.retain_dispatch(pending.scale, pending.viewport, false);
        }
        let Some(dispatch) = self.dispatch.take() else {
            return false;
        };
        if dispatch.handler.dispatch_pointer_move(self, &pending.event) {
            self.finish_handled_event();
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
    }

    pub(crate) fn refresh_title(&self) {
        if let RenderState::Active { window, .. } = &self.state {
            window.set_title(&self.title());
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::WindowExtMacOS;
                window.set_document_edited(self.model.history.dirty());
                macos_window::set_represented(window, self.doc_path.as_deref());
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
            save: self.model.history.dirty() || self.doc_path.is_none(),
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
            let before = self.model.doc.clone();
            let previous = self
                .model
                .selection
                .as_ref()
                .map(|selection| selection.path().to_vec());
            self.model.doc.root = Some(value);
            self.model.history.record(before, previous);
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
            self.model.selection =
                Some(selection::Selection::edge(&self.sources(), path).with_root(root));
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
            let before = self.model.doc.clone();
            let previous = self
                .model
                .selection
                .as_ref()
                .map(|selection| selection.path().to_vec());
            self.model.doc.root = Some(value);
            self.model.history.record(before, previous);
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
                let _ = self.proxy.send_event(UserEvent::Command(command));
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
        let current = self.model.doc.clone();
        let selection = edge_path(&self.model.selection);
        let root = self
            .model
            .selection
            .as_ref()
            .map(selection::Selection::root)
            .filter(|root| self.model.workspace.view(root).is_some())
            .cloned()
            .unwrap_or_else(|| self.model.workspace.document_root().clone());
        let restored = if back {
            self.model.history.undo(current, selection)
        } else {
            self.model.history.redo(current, selection)
        };
        if let Some((doc, restore)) = restored {
            self.model.doc = doc;
            self.model.selection = restore
                .map(|path| selection::Selection::edge(&self.sources(), path).with_root(root));
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
                    self.model.history.mark_saved();
                    // A run must not straddle the save mark, or edits
                    // after it would coalesce into a pre-save step.
                    selection::break_edit_run(self.model.selection.as_mut());
                    self.adopt_doc_path(canonical(path));
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
    #[cfg_attr(not(any(target_arch = "wasm32", target_os = "ios")), allow(dead_code))]
    pub(crate) fn adopt_model(
        &mut self,
        doc: gid::Document,
        path: Option<PathBuf>,
        text_binders: gid_text::Binders,
    ) {
        self.text_binders = text_binders;
        let view = self.model.view;
        self.model = Model {
            doc,
            selection: None,
            history: history::History::default(),
            view,
            workspace: workspace::Workspace::default(),
        };
        self.hover = None;
        self.point = None;
        self.scrub = None;
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

    /// Every document lives in its own window: New, Open, and the
    /// examples each open one; Quit drains them.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn run_app_command(&mut self, event_loop: &ActiveEventLoop, command: AppCommand) {
        match command {
            AppCommand::New => self.open_editor(
                event_loop,
                gid::Document {
                    root: None,
                    cells: gid::Cells::new(),
                },
                None,
                gid_text::Binders::new(),
            ),
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
            AppCommand::Close => {
                if let Some(index) = self.focused_index() {
                    self.request_discard(event_loop, index, AfterDiscard::CloseWindow);
                }
            }
            AppCommand::Quit => self.begin_quit(event_loop),
            AppCommand::Example(example) => match gid_text::parse(example.source()) {
                Ok((doc, binders)) => self.open_editor(event_loop, doc, None, binders),
                Err(error) => panic!("built-in example failed to parse: {error}"),
            },
        }
    }

    /// One canvas: replace the document in place, gated on unsaved
    /// changes.
    #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
    fn run_app_command(&mut self, event_loop: &ActiveEventLoop, command: AppCommand) {
        let Some(index) = self.focused_index() else {
            return;
        };
        match command {
            AppCommand::New => self.request_discard(event_loop, index, AfterDiscard::New),
            AppCommand::Quit => self.request_discard(event_loop, index, AfterDiscard::Quit),
            AppCommand::Example(example) => {
                self.request_discard(event_loop, index, AfterDiscard::Example(example))
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
    pub(crate) fn request_discard(
        &mut self,
        event_loop: &ActiveEventLoop,
        index: usize,
        then: AfterDiscard,
    ) {
        if !self.editors[index].model.history.dirty() {
            self.proceed(event_loop, index, then);
            return;
        }
        if self.editors[index].pending_discard.is_some() {
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
            let proxy = editor.proxy.clone();
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
    #[cfg_attr(any(target_os = "macos", target_os = "linux"), allow(unused_variables))]
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
            #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
            AfterDiscard::New => self.editors[index].adopt_model(
                gid::Document {
                    root: None,
                    cells: gid::Cells::new(),
                },
                None,
                gid_text::Binders::new(),
            ),
            #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
            AfterDiscard::Quit => event_loop.exit(),
            #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
            AfterDiscard::Example(example) => match gid_text::parse(example.source()) {
                Ok((doc, binders)) => self.editors[index].adopt_model(doc, None, binders),
                Err(error) => panic!("built-in example failed to parse: {error}"),
            },
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
        let pending = editor
            .pending_paint
            .take()
            .filter(|pending| pending.scale == scale && pending.viewport == viewport);
        let (renders, hovered_secondary, hovered_trace) = match pending {
            Some(PendingPaint {
                renders,
                hovered_secondary,
                hovered_trace,
                ..
            }) => (renders, hovered_secondary, hovered_trace),
            None => {
                let Frame {
                    dispatch,
                    renders,
                    hovered_secondary,
                    hovered_trace,
                } = editor.build_frame(scale, viewport);
                editor.last_descends = dispatch.descends.clone();
                editor.dispatch = Some(dispatch);
                (renders, hovered_secondary, hovered_trace)
            }
        };
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
        let pending = editor
            .pending_paint
            .take()
            .filter(|pending| pending.scale == scale && pending.viewport == viewport);
        let (renders, hovered_secondary, hovered_trace) = match pending {
            Some(PendingPaint {
                renders,
                hovered_secondary,
                hovered_trace,
                ..
            }) => (renders, hovered_secondary, hovered_trace),
            None => {
                let Frame {
                    dispatch,
                    renders,
                    hovered_secondary,
                    hovered_trace,
                } = editor.build_frame(scale, viewport);
                editor.last_descends = dispatch.descends.clone();
                editor.dispatch = Some(dispatch);
                (renders, hovered_secondary, hovered_trace)
            }
        };
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
