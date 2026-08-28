//! Window shell: winit + Vello plumbing around pure frame drawing.
//! `app_view` renders to any puri `Canvas`; here its deferred ink
//! streams into vello.

mod annotations;
mod commands;
mod completion;
mod filter;
mod frame;
mod gid_text;
#[cfg(test)]
mod grap_examples;
mod history;
mod hover;
mod identity;
#[cfg(target_os = "macos")]
mod macos_surface;
#[cfg(target_os = "macos")]
mod macos_menu;
#[cfg(target_os = "macos")]
mod macos_window;
mod menu;
mod model;
mod modifiers;
mod navigate;
mod placed;
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

use crate::frame::{Dispatch, FrameDisposition, Frame, Hovered, Paint, frame_disposition};
use crate::model::{Model, ViewFlags};
use kurbo::{Point, Rect, Size};
use peniko::{Brush, Color};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use parley::{FontContext, LayoutContext};
use puri::edit::TextClipboard;
use puri::handler::ImeEvent;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};
use ui_events::pointer::{PointerEvent, PointerScrollEvent, PointerType, PointerUpdate};
use ui_events::ScrollDelta;
use ui_events_winit::{WindowEventReducer, WindowEventTranslation};
#[cfg(not(target_arch = "wasm32"))]
use vello::util::{RenderContext, RenderSurface};
#[cfg(not(target_arch = "wasm32"))]
use vello::wgpu::{self, CurrentSurfaceTexture};
#[cfg(not(target_arch = "wasm32"))]
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
#[cfg(not(target_arch = "wasm32"))]
use winit::dpi::LogicalSize;
use winit::dpi::PhysicalPosition;
use winit::event::{Ime, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{CursorIcon, Window, WindowId};
#[cfg(target_arch = "wasm32")]
use winit::platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys, WindowExtWebSys};
#[cfg(target_arch = "wasm32")]
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

/// Everything arriving through the event-loop proxy.
pub(crate) enum UserEvent {
    #[cfg(target_os = "macos")]
    MacMenu(macos_menu::Event),
    Menu(menu::Selection),
    #[cfg(not(target_arch = "wasm32"))]
    Discard(bool),
}

/// The action a discard confirmation gates. One at a time: requests
/// while a sheet is up are dropped.
pub(crate) enum AfterDiscard {
    New,
    #[cfg(not(target_arch = "wasm32"))]
    Open,
    Quit,
    Example(Example),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Example {
    Sample,
    Grap,
    IopTree,
}

impl Example {
    fn source(self) -> &'static str {
        match self {
            Self::Sample => include_str!("../../examples/sample.gid"),
            Self::Grap => include_str!("../../examples/grap-demo.gid"),
            Self::IopTree => include_str!("../../examples/iop-tree.gid"),
        }
    }
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
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const CLIPBOARD_FORMAT: &str = "com.progred.value";

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct SystemTextClipboard;

#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub(crate) struct SystemTextClipboard {
    text: Option<String>,
    structure: Option<gid::Value>,
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(target_arch = "wasm32")]
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

pub(crate) struct App {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) context: RenderContext,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) renderers: Vec<Option<Renderer>>,
    pub(crate) state: RenderState,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) scene: Scene,
    pub(crate) font_cx: FontContext,
    pub(crate) layout_cx: LayoutContext<Brush>,
    pub(crate) text_clipboard: SystemTextClipboard,
    pub(crate) text_cache: puri::text::TextCache,
    pub(crate) drawing_memos: HashMap<workspace::Root, projection::DrawingMemo>,
    /// Editor configuration shared by every document loaded into the
    /// app: library cells, Rust functions, and composed projection.
    pub(crate) stack: stack::Stack<App>,
    pub(crate) model: Model,
    /// Where the document lives; `None` is untitled until the first
    /// save asks for a path.
    pub(crate) doc_path: Option<PathBuf>,
    /// The text bridge's file-local binder table, surviving load → save
    /// so spellings round-trip; never part of the model, invisible
    /// in the document.
    pub(crate) text_binders: gid_text::Binders,
    #[cfg(target_os = "macos")]
    pub(crate) native_menu: macos_menu::Menu,
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
    /// Pointer-driven traversal of nonlocal graph links. Keyboard
    /// shortcuts using the same modifier do not enter this mode.
    pub(crate) linking: bool,
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
    /// Geometry from the last minted frame, so projection key
    /// handlers can land a delete the same way the shell fallback
    /// does.
    pub(crate) last_descends: Vec<navigate::Descend<App>>,
    pub(crate) reducer: WindowEventReducer,
    /// Routes the discard sheet's answer back into the loop.
    pub(crate) proxy: winit::event_loop::EventLoopProxy<UserEvent>,
    pub(crate) pending_discard: Option<AfterDiscard>,
}

pub(crate) fn menu_height(scale: f64) -> f64 {
    if menu::DRAWN { menu::bar_height(scale) } else { 0.0 }
}

pub(crate) fn content_viewport(viewport: Size, scale: f64) -> Rect {
    Rect::new(
        0.0,
        menu_height(scale).min(viewport.height),
        viewport.width,
        viewport.height,
    )
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
        Some(current) if current.stage() == selection::Stage::Edge => {
            Some(current.path().to_vec())
        }
        _ => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn text_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("GID", &["gid"])
}

impl ApplicationHandler<UserEvent> for App {
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        if self.flush_pending_continuous()
            && let RenderState::Active { window, .. } = &self.state
        {
            window.request_redraw();
        }
        match event {
            #[cfg(target_os = "macos")]
            UserEvent::MacMenu(event) => {
                if let Some(selection) = self.native_menu.selection(&event) {
                    self.handle_menu_selection(event_loop, selection);
                }
            }
            UserEvent::Menu(selection) => self.handle_menu_selection(event_loop, selection),
            #[cfg(not(target_arch = "wasm32"))]
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
            let attributes = Window::default_attributes().with_title(self.title());
            #[cfg(not(target_arch = "wasm32"))]
            let attributes = attributes.with_inner_size(LogicalSize::new(900, 640));
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
            // The sole current window occupies session slot zero. A
            // multi-window session will supply distinct persistent IDs.
            macos_window::autosave_frame(&window, "window-0");
            Arc::new(window)
        });

        #[cfg(not(target_arch = "wasm32"))]
        {
        let size = window.inner_size();
        #[cfg(target_os = "macos")]
        let surface_future = self.context.create_render_surface(
            macos_surface::create(&self.context.instance, &window),
            size.width,
            size.height,
            wgpu::PresentMode::AutoVsync,
        );
        #[cfg(not(target_os = "macos"))]
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

        #[cfg(target_arch = "wasm32")]
        {
            let canvas = window.canvas().expect("Winit web canvas");
            let context = canvas
                .get_context("2d")
                .expect("Canvas2D lookup")
                .expect("Canvas2D context")
                .dyn_into::<CanvasRenderingContext2d>()
                .expect("CanvasRenderingContext2D");
            self.state = RenderState::Active {
                canvas,
                context,
                window,
            };
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.flush_pending_continuous();
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
        // Coalesce by interaction semantics, not by comparing
        // coordinates or trying to recognize Winit's macOS-generated
        // refresh. Real unpressed motion is sampled at frame rate too;
        // the newest position is the frame input. A future consumer
        // that needs the intervening samples can receive them through
        // `PointerUpdate::coalesced` without forcing intermediate
        // projection/layout passes.
        let continuous_pointer = matches!(&event, WindowEvent::CursorMoved { .. }) && !self.pressed;
        if !matches!(&event, WindowEvent::MouseWheel { .. })
            && !continuous_pointer
            && self.flush_pending_continuous()
        {
            window.request_redraw();
        }

        if let WindowEvent::ModifiersChanged(state) = &event
            && self.linking
            && !modifiers::link(
                &ui_events_winit::keyboard::from_winit_modifier_state(state.state()),
            )
        {
            self.linking = false;
            let size = window.inner_size();
            self.retain_dispatch(
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
            let translation = self.reducer.reduce(scale, &event);
            let previous_cursor = self.cursor;
            if let Some(WindowEventTranslation::Pointer(pointer)) = &translation
                && let Some(position) = pointer_position(pointer)
            {
                self.cursor = position;
            }
            // Touch has no preceding hover motion. Mint once at the
            // contact point before dispatch so the ordinary activate
            // fallback sees exactly the target a mouse click would.
            if let Some(WindowEventTranslation::Pointer(PointerEvent::Down(button))) = &translation
                && button.pointer.pointer_type == PointerType::Touch
            {
                let size = window.inner_size();
                self.pointer = Some(Point::new(
                    button.state.position.x,
                    button.state.position.y,
                ));
                self.retain_dispatch(
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
                if let Some(pending) = self.queue_scroll(next) {
                    self.dispatch_scroll_batch(pending);
                }
                window.request_redraw();
            } else if let Some(WindowEventTranslation::Pointer(PointerEvent::Move(update))) =
                &translation
                && !self.pressed
            {
                let size = window.inner_size();
                let position = Point::new(
                    update.current.position.x,
                    update.current.position.y,
                );
                self.pointer = Some(position);
                self.linking = modifiers::link(&update.current.modifiers);
                self.pending_pointer = Some(PendingPointer {
                    event: update.clone(),
                    scale,
                    viewport: Size::new(size.width as f64, size.height as f64),
                });
                window.request_redraw();
            } else if (ime.is_some() || translation.is_some())
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
                            || self.delete_key(&dispatch.descends, &key_event)
                            || self.insert_key(&dispatch.descends, &dispatch.popup, &key_event)
                            || self.collapse_key(&key_event)
                            || match navigate::step_selection(
                                &dispatch.descends,
                                Some(
                                    self.model
                                        .selection
                                        .as_ref()
                                        .map(selection::Selection::root)
                                        .unwrap_or_else(|| self.model.workspace.document_root()),
                                ),
                                self.model.selection.as_ref(),
                                dispatch.line,
                                &key_event,
                            ) {
                                Some(target) => {
                                    let select = target.select.clone();
                                    select(self);
                                    if let Some(selection) = &mut self.model.selection {
                                        selection::seed_from_arrow(selection, &key_event);
                                    }
                                    true
                                }
                                None => false,
                            }
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Down(button)))) => {
                        let position =
                            Point::new(button.state.position.x, button.state.position.y);
                        self.pointer = Some(position);
                        self.pressed = true;
                        let event_root = dispatch
                            .view_regions
                            .iter()
                            .rev()
                            .find(|region| region.rect.contains(position))
                            .map(|region| region.root.clone());
                        let raw = dispatch.handler.dispatch_pointer_down(self, &button);
                        if raw || !puri::interact::is_primary_contact(&button) {
                            raw
                        } else if let Some(target) = self.hover.clone() {
                            if modifiers::pick(&button.state.modifiers) {
                                placed::dispatch_target(
                                    &dispatch.picks,
                                    self,
                                    event_root.as_ref(),
                                    &target,
                                )
                            } else {
                                placed::dispatch_target(
                                    &dispatch.activations,
                                    self,
                                    event_root.as_ref(),
                                    &target,
                                )
                            }
                        } else {
                            self.model.selection.take().is_some()
                        }
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Move(update)))) => {
                        // Pointer position is frame input. Unpressed
                        // motion remints even when no event handler
                        // consumes it; pressed gestures freeze hover
                        // while their ordinary drag handlers run.
                        let position = Point::new(
                            update.current.position.x,
                            update.current.position.y,
                        );
                        self.pointer = Some(position);
                        frame_input_changed = !self.pressed;
                        let moved = dispatch.handler.dispatch_pointer_move(self, &update);
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
                            dispatch.handler.dispatch_scroll(self, &scroll).handled()
                        }
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Up(button)))) => {
                        let position =
                            Point::new(button.state.position.x, button.state.position.y);
                        self.pointer = Some(position);
                        self.pressed = false;
                        frame_input_changed = true;
                        dispatch.handler.dispatch_pointer_up(self, &button)
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Leave(_)))) => {
                        self.pointer = None;
                        self.linking = false;
                        self.pressed = false;
                        frame_input_changed = true;
                        self.model.workspace.cancel_resize()
                    }
                    (None, Some(WindowEventTranslation::Pointer(PointerEvent::Cancel(pointer)))) => {
                        self.pointer = None;
                        self.pressed = false;
                        frame_input_changed = true;
                        let handled = dispatch.handler.dispatch_pointer_cancel(self, &pointer);
                        let resize_cancelled = self.model.workspace.cancel_resize();
                        handled || resize_cancelled
                    }
                    _ => false,
                };
                if handled {
                    self.finish_handled_event();
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

            WindowEvent::Resized(size) => {
                let valid = size.width != 0 && size.height != 0;
                #[cfg(not(target_arch = "wasm32"))]
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
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    #[cfg(not(target_arch = "wasm32"))]
    let doc_path = std::env::args().nth(1).map(PathBuf::from);
    #[cfg(target_arch = "wasm32")]
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
    let native_menu = macos_menu::Menu::new();
    #[cfg(target_os = "macos")]
    macos_menu::route_events(proxy.clone());

    #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
    let mut app = App {
        #[cfg(not(target_arch = "wasm32"))]
        context: RenderContext::new(),
        #[cfg(not(target_arch = "wasm32"))]
        renderers: vec![],
        state: RenderState::Suspended(None),
        #[cfg(not(target_arch = "wasm32"))]
        scene: Scene::new(),
        font_cx: font_context(),
        layout_cx: LayoutContext::new(),
        #[cfg(not(target_arch = "wasm32"))]
        text_clipboard: SystemTextClipboard,
        #[cfg(target_arch = "wasm32")]
        text_clipboard: SystemTextClipboard::default(),
        text_cache: puri::text::TextCache::default(),
        drawing_memos: HashMap::new(),
        stack: stack::load(),
        model: Model {
            doc,
            selection: None,
            history: history::History::default(),
            view: ViewFlags::default(),
            workspace: workspace::Workspace::default(),
        },
        doc_path,
        text_binders: binders,
        #[cfg(target_os = "macos")]
        native_menu,
        menu: menu::State::default(),
        cursor: Point::ZERO,
        cursor_icon: CursorIcon::Default,
        pointer: None,
        hover: None,
        linking: false,
        pressed: false,
        revealed: None,
        dispatch: None,
        pending_paint: None,
        pending_scroll: None,
        pending_pointer: None,
        last_descends: Vec::new(),
        reducer: WindowEventReducer::default(),
        proxy,
        pending_discard: None,
    };

    #[cfg(not(target_arch = "wasm32"))]
    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");
    #[cfg(target_arch = "wasm32")]
    event_loop.spawn_app(app);
}

impl App {
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
        let library = &self.stack.library;
        let foreign = &self.stack.foreign;
        let model = &mut self.model;
        if let Some(selection) = &mut model.selection {
            let before = model.doc.clone();
            if selection::write_through(&mut model.doc, library, foreign, selection) {
                let path = selection.path().to_vec();
                model.history.record(before, Some(path));
                self.refresh_title();
            }
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
            library: &self.stack.library,
        }
    }

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

    fn sync_cursor(&mut self, window: &Window) {
        let next = cursor_icon(self.hover.as_ref());
        if next != self.cursor_icon {
            window.set_cursor(next);
            self.cursor_icon = next;
        }
    }

    /// Menu enablement follows the model: gray what can't act. Save
    /// stays live for untitled documents — it defers to the save
    /// panel, per platform convention.
    pub(crate) fn sync_menus(&self) {
        #[cfg(target_os = "macos")]
        self.native_menu.sync(
            self.menu_availability(),
            self.model.view,
            self.model
                .workspace
                .selected_or_document(
                    self.model.selection.as_ref().map(selection::Selection::root),
                )
                .projection
                == workspace::Projection::Raw,
        );
    }

    pub(crate) fn menu_availability(&self) -> menu::Availability {
        let selected_root = self.model.selection.as_ref().map(selection::Selection::root);
        menu::Availability {
            #[cfg(not(target_arch = "wasm32"))]
            save: self.model.history.dirty() || self.doc_path.is_none(),
            undo: self.model.history.can_undo(),
            redo: self.model.history.can_redo(),
            open_pane: self.selected_cell_for_pane().is_some(),
            move_up: selected_root.is_some_and(|root| {
                self.model.workspace.can_move(root, workspace::Move::Up)
            }),
            move_down: selected_root.is_some_and(|root| {
                self.model.workspace.can_move(root, workspace::Move::Down)
            }),
            move_left: selected_root.is_some_and(|root| {
                self.model.workspace.can_move(root, workspace::Move::Left)
            }),
            move_right: selected_root.is_some_and(|root| {
                self.model.workspace.can_move(root, workspace::Move::Right)
            }),
        }
    }

    fn selected_cell_for_pane(&self) -> Option<(gid::CellId, gid::Path)> {
        let current = self.model.selection.as_ref()?;
        let path = current.path();
        let sources = self.sources();
        if let Some(cell) = sources.resolve(path).and_then(gid::Value::as_cell) {
            return Some((cell, path.to_vec()));
        }
        let follow = selection::last_follow(path)?;
        let anchor = path[..follow].to_vec();
        let cell = sources.resolve(&anchor)?.as_cell()?;
        Some((cell, anchor))
    }

    fn open_selected_in_pane(&mut self, side: workspace::Side) -> bool {
        let Some((cell, anchor)) = self.selected_cell_for_pane() else {
            return false;
        };
        let root = self.model.workspace.open_cell(side, cell, anchor.clone());
        self.model.selection = Some(
            selection::Selection::edge(&self.sources(), anchor).with_root(root),
        );
        true
    }

    pub(crate) fn handle_menu_selection(
        &mut self,
        event_loop: &ActiveEventLoop,
        selection: menu::Selection,
    ) {
        match selection {
            menu::Selection::New => self.request_discard(event_loop, AfterDiscard::New),
            #[cfg(not(target_arch = "wasm32"))]
            menu::Selection::Open => self.request_discard(event_loop, AfterDiscard::Open),
            #[cfg(not(target_arch = "wasm32"))]
            menu::Selection::Save => self.menu_save(false),
            #[cfg(not(target_arch = "wasm32"))]
            menu::Selection::SaveAs => self.menu_save(true),
            menu::Selection::Quit => self.request_discard(event_loop, AfterDiscard::Quit),
            menu::Selection::ExampleSample => {
                self.request_discard(event_loop, AfterDiscard::Example(Example::Sample))
            }
            menu::Selection::ExampleGrap => {
                self.request_discard(event_loop, AfterDiscard::Example(Example::Grap))
            }
            menu::Selection::ExampleIopTree => {
                self.request_discard(event_loop, AfterDiscard::Example(Example::IopTree))
            }
            menu::Selection::Undo => self.step_history(true),
            menu::Selection::Redo => self.step_history(false),
            menu::Selection::OpenPaneLeft => {
                self.open_selected_in_pane(workspace::Side::Left);
            }
            menu::Selection::OpenPaneRight => {
                self.open_selected_in_pane(workspace::Side::Right);
            }
            menu::Selection::MovePaneUp
            | menu::Selection::MovePaneDown
            | menu::Selection::MovePaneLeft
            | menu::Selection::MovePaneRight => {
                let direction = match selection {
                    menu::Selection::MovePaneUp => workspace::Move::Up,
                    menu::Selection::MovePaneDown => workspace::Move::Down,
                    menu::Selection::MovePaneLeft => workspace::Move::Left,
                    menu::Selection::MovePaneRight => workspace::Move::Right,
                    _ => unreachable!(),
                };
                if let Some(root) = self
                    .model
                    .selection
                    .as_ref()
                    .map(selection::Selection::root)
                    .cloned()
                {
                    self.model.workspace.move_pane(&root, direction);
                }
            }
            menu::Selection::Raw => {
                let selected = self
                    .model
                    .selection
                    .as_ref()
                    .map(selection::Selection::root)
                    .cloned();
                self.model.workspace.toggle_projection(selected.as_ref());
            }
            menu::Selection::DebugGeometry => {
                self.model.view.debug_geometry = !self.model.view.debug_geometry
            }
        }
        if matches!(
            selection,
            menu::Selection::OpenPaneLeft
                | menu::Selection::OpenPaneRight
                | menu::Selection::MovePaneUp
                | menu::Selection::MovePaneDown
                | menu::Selection::MovePaneLeft
                | menu::Selection::MovePaneRight
                | menu::Selection::Raw
                | menu::Selection::DebugGeometry
        ) {
            self.pending_paint = None;
            if let RenderState::Active { window, .. } = &self.state {
                window.request_redraw();
            }
        }
    }

    pub(crate) fn choose_menu(&mut self, selection: menu::Selection) {
        self.menu.close();
        let _ = self.proxy.send_event(UserEvent::Menu(selection));
    }

    pub(crate) fn menu_key(&mut self, event: &KeyboardEvent) -> bool {
        // The native menu owns its own shortcuts; only the drawn
        // menu routes keys here.
        if !menu::DRAWN {
            return false;
        }
        let open = self.menu.open().is_some();
        if open
            && event.state.is_down()
            && modifiers::plain(&event.modifiers)
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
            self.model.selection = restore.map(|path| {
                selection::Selection::edge(&self.sources(), path).with_root(root)
            });
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
        #[cfg(target_arch = "wasm32")]
        {
            let accepted = web_sys::window()
                .and_then(|window| {
                    window
                        .confirm_with_message("Discard unsaved changes?")
                        .ok()
                })
                .unwrap_or(false);
            if accepted {
                self.proceed(event_loop, then);
            }
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
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
    }

    /// The action a confirmed (or unneeded) discard proceeds to.
    pub(crate) fn proceed(&mut self, event_loop: &ActiveEventLoop, then: AfterDiscard) {
        match then {
            AfterDiscard::New => self.adopt_model(
                gid::Document {
                    root: None,
                    cells: gid::Cells::new(),
                },
                None,
                gid_text::Binders::new(),
            ),
            #[cfg(not(target_arch = "wasm32"))]
            AfterDiscard::Open => {
                if let Some(path) = text_dialog().pick_file() {
                    match text_store::load(&path) {
                        Ok((doc, binders)) => self.adopt_model(doc, Some(path), binders),
                        Err(error) => {
                            eprintln!("failed to open {}: {error}", path.display());
                        }
                    }
                }
            }
            AfterDiscard::Quit => event_loop.exit(),
            AfterDiscard::Example(example) => match gid_text::parse(example.source()) {
                Ok((doc, binders)) => self.adopt_model(doc, None, binders),
                Err(error) => panic!("built-in example failed to parse: {error}"),
            },
        }
    }

    /// Save saves in place, or asks for a path when untitled; save-as
    /// always asks. Write-through editing means the GID document is always
    /// current, so there is nothing to flush first. A cancelled dialog
    /// saves nothing.
    #[cfg(not(target_arch = "wasm32"))]
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

    #[cfg(not(target_arch = "wasm32"))]
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
    #[cfg(not(target_arch = "wasm32"))]
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

        self.sync_menus();
        let viewport = Size::new(width as f64, height as f64);
        self.scene.reset();
        let pending = self
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
                } = self.build_frame(scale, viewport);
                self.last_descends = dispatch.descends.clone();
                self.dispatch = Some(dispatch);
                (renders, hovered_secondary, hovered_trace)
            }
        };
        self.sync_cursor(&window);
        let ink = placed::Ink {
            hovered: self.hover.as_ref(),
            hovered_secondary: hovered_secondary.as_ref(),
            hovered_trace: hovered_trace.as_ref(),
            debug_geometry: self.model.view.debug_geometry,
        };
        let mut paint = Paint {
            scene: std::mem::replace(&mut self.scene, Scene::new()),
        };
        for render in renders {
            render(&mut paint, ink);
        }
        self.scene = paint.scene;

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

    /// The browser runs the same deferred frame ink directly into
    /// Canvas2D. Layout and event dispatch are shared with desktop;
    /// only this final interpreter differs.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn redraw(&mut self) {
        let RenderState::Active {
            canvas,
            context,
            window,
        } = &self.state
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

        self.sync_menus();
        let viewport = Size::new(width as f64, height as f64);
        let pending = self
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
                } = self.build_frame(scale, viewport);
                self.last_descends = dispatch.descends.clone();
                self.dispatch = Some(dispatch);
                (renders, hovered_secondary, hovered_trace)
            }
        };
        self.sync_cursor(&window);
        let ink = placed::Ink {
            hovered: self.hover.as_ref(),
            hovered_secondary: hovered_secondary.as_ref(),
            hovered_trace: hovered_trace.as_ref(),
            debug_geometry: self.model.view.debug_geometry,
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
