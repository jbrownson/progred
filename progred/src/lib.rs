//! Window shell: translate platform input, schedule updates, and present
//! the editor frame through Vello or Canvas2D.

mod annotations;
mod app;
mod clipboard;
mod command;
mod commands;
mod completion;
mod computations;
mod display;
mod editing;
mod editor;
mod filter;
mod fonts;
mod frame;
mod gesture;
mod gid_text;
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
#[cfg(feature = "cam-profile")]
pub mod orbit_profile;
mod placed;
mod platform;
mod projection;
mod render;
#[cfg(test)]
mod sample;
mod selection;
#[cfg(test)]
mod shell_tests;
mod site;
mod sources;
mod spine;
mod stack;
mod styles;
#[cfg(test)]
mod test_examples;
#[cfg(test)]
mod test_values;
mod text_store;
mod timers;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(any(test, target_arch = "wasm32"))]
mod web_embed;
#[cfg(target_arch = "wasm32")]
mod web_input;
#[cfg(target_arch = "wasm32")]
pub mod web_render;
#[cfg(any(test, target_arch = "wasm32"))]
mod web_scroll;
#[cfg(target_arch = "wasm32")]
pub mod web_worker;
mod workspace;

#[cfg(all(feature = "cam-profile", target_arch = "wasm32"))]
pub use libraries::fidget::mesh::performance::take_profile_mesh;
#[cfg(feature = "cam-profile")]
pub use libraries::toolpath::performance::profile_cam;

use crate::command::Command;
use crate::frame::Hovered;
use app::App;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use app::QuitState;
use editor::new_editor;
pub(crate) use editor::{Editor, EditorRunner};
#[cfg(test)]
pub(crate) use editor::{test_editor, test_editor_with_stack, test_root};
use fonts::font_context;
use kurbo::{Rect, Size};
use std::path::PathBuf;
#[cfg(target_arch = "wasm32")]
use web::WEB_PROXY;

use ui_events::pointer::{PointerEvent, PointerId, PointerInfo, PointerType};
use ui_events_winit::{WindowEventReducer, WindowEventTranslation};
#[cfg(not(target_arch = "wasm32"))]
use vello::util::RenderContext;
use winit::event::WindowEvent;
use winit::event_loop::EventLoop;
use winit::window::CursorIcon;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use winit::window::WindowId;

/// Everything arriving through the event-loop proxy.
pub(crate) enum UserEvent {
    ComputationFinished,
    #[cfg(target_arch = "wasm32")]
    BrowserFocusChanged,
    #[cfg(target_arch = "wasm32")]
    BrowserModifiersChanged(winit::keyboard::ModifiersState),
    #[cfg(target_arch = "wasm32")]
    PaletteChanged(styles::Palette),
    #[cfg(target_os = "macos")]
    NativeMenu(native_menu::Event),
    Command(Command),
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Discard {
        window: WindowId,
        accepted: bool,
    },
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

pub fn run() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let doc_path = std::env::args().nth(1).map(PathBuf::from).map(canonical);
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

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let drawn_menu = platform::DRAWN_MENU || std::env::var_os("PROGRED_DRAWN_MENU").is_some();
    #[cfg(target_arch = "wasm32")]
    let drawn_menu = platform::DRAWN_MENU;
    run_document(
        doc,
        binders,
        doc_path,
        drawn_menu,
        stack::load(),
        modifiers::native(),
        styles::Theme::Light.palette(),
        #[cfg(target_arch = "wasm32")]
        None,
    );
}

fn run_document(
    doc: gid::Document,
    binders: gid_text::Binders,
    doc_path: Option<PathBuf>,
    drawn_menu: bool,
    stack: stack::Stack<Editor>,
    command_modifier: puri::keyboard::CommandModifier,
    palette: styles::Palette,
    #[cfg(target_arch = "wasm32")] on_change: Option<web_sys::js_sys::Function>,
) {
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

    let fonts = font_context();
    #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
    let mut app = App {
        #[cfg(target_arch = "wasm32")]
        web_observer: on_change.map(web_embed::Observer::new),
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
        #[cfg(target_os = "macos")]
        appearance: None,
        proxy: proxy.clone(),
        drawn_menu,
        focused: None,
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        quit: QuitState::Idle,
        #[cfg(target_os = "macos")]
        cascade: macos_window::initial_cascade(),
        editors: vec![EditorRunner::new(new_editor(
            palette,
            command_modifier,
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
    web_input::spawn(app, event_loop);
}
