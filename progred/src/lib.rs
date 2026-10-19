//! Window shell: translate platform input, schedule updates, and present
//! the editor frame through Vello or Canvas2D.

mod annotations;
mod app;
#[cfg(feature = "cam-profile")]
mod cam_profile;
mod clipboard;
mod command;
mod completion;
mod computations;
#[cfg(any(test, feature = "control"))]
pub mod control;
mod display;
#[cfg(any(test, feature = "control"))]
mod driver;
mod edit_keys;
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
mod library;
mod location;
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
#[cfg(test)]
mod perf;
#[cfg(not(target_arch = "wasm32"))]
mod platform;
mod projection;
#[cfg(test)]
mod sample;
mod selection;
#[cfg(test)]
mod shell_tests;
mod site;
mod sources;
mod spine;
mod stack;
#[cfg(test)]
mod svg;
#[cfg(test)]
mod test_examples;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod test_values;
#[cfg(not(target_arch = "wasm32"))]
mod text_store;
mod timers;
#[cfg(test)]
mod tutorials;
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

#[cfg(feature = "cam-profile")]
pub use cam_profile::profile_cam;
#[cfg(all(feature = "cam-profile", target_arch = "wasm32"))]
pub use libraries::fidget::mesh::performance::take_profile_mesh;

use crate::command::Command;
use app::App;
use editor::new_editor;
pub(crate) use editor::{Editor, EditorRunner};
use fonts::font_context;
use std::path::PathBuf;
#[cfg(test)]
pub(crate) use test_support::{test_editor, test_editor_with_stack, test_root};
#[cfg(target_arch = "wasm32")]
use web::WEB_PROXY;

use winit::event_loop::EventLoop;
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
    PaletteChanged(display::widget::style::Palette),
    /// The libraries whose projections the embedding page wants drawn, and
    /// whether names show.
    #[cfg(target_arch = "wasm32")]
    ProjectionsChanged {
        projections: String,
        names: bool,
    },
    #[cfg(target_os = "macos")]
    NativeMenu(native_menu::Event),
    /// A file the browser's Open picked, as its name and text.
    #[cfg(target_arch = "wasm32")]
    Opened {
        name: String,
        source: String,
    },
    Command(Command),
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Discard {
        window: WindowId,
        accepted: bool,
    },
}

/// One canonical spelling per document, so every identity, such as a
/// window's remembered frame, agrees however the path was written.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn canonical(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) fn text_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("GID", &["gid"])
}

/// Opens the document the command line names, or an untitled one, in the
/// desktop app. The browser starts its editor through `web::start_editor`.
#[cfg(not(target_arch = "wasm32"))]
pub fn run() {
    let doc_path = std::env::args().nth(1).map(PathBuf::from).map(canonical);
    // A given-but-missing path is a new document there; no path is
    // untitled until the first save asks. A file that exists but does
    // not parse is refused, so no save replaces it.
    let (doc, binders) = match &doc_path {
        Some(path) if path.exists() => text_store::load(path).unwrap_or_else(|error| {
            eprintln!("failed to load {}: {error}", path.display());
            std::process::exit(1);
        }),
        _ => (gid::Document::default(), gid_text::Binders::new()),
    };
    let drawn_menu = platform::DRAWN_MENU || std::env::var_os("PROGRED_DRAWN_MENU").is_some();
    run_document(
        doc,
        binders,
        doc_path,
        drawn_menu,
        stack::load(),
        modifiers::native(),
        display::widget::style::Theme::Light.palette(),
    );
}

fn run_document(
    doc: gid::Document,
    binders: gid_text::Binders,
    doc_path: Option<PathBuf>,
    drawn_menu: bool,
    stack: stack::Stack,
    command_modifier: puri::keyboard::CommandModifier,
    palette: display::widget::style::Palette,
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
    let fonts = font_context();
    #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
    let mut app = App::new(
        proxy.clone(),
        new_editor(
            palette,
            command_modifier,
            drawn_menu,
            stack.clone(),
            fonts.clone(),
            doc,
            doc_path,
            binders,
            Some(proxy),
        ),
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        stack,
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        fonts,
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        drawn_menu,
        #[cfg(target_arch = "wasm32")]
        on_change,
    );

    #[cfg(not(target_arch = "wasm32"))]
    event_loop
        .run_app(&mut app)
        .expect("Couldn't run event loop");
    #[cfg(target_arch = "wasm32")]
    web_input::spawn(app, event_loop);
}
