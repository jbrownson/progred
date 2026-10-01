//! Browser entry points called from the embedding page.

use crate::{UserEvent, gid_text, run_document, styles, web_embed};
use wasm_bindgen::JsCast;
use winit::platform::web::WindowExtWebSys;
use winit::window::Window;

thread_local! {
    pub(crate) static WEB_PROXY: std::cell::RefCell<Option<winit::event_loop::EventLoopProxy<UserEvent>>> =
        const { std::cell::RefCell::new(None) };
}

pub(crate) fn browser_editor_focused(window: &Window) -> bool {
    window.canvas().is_some_and(|canvas| {
        canvas.owner_document().is_some_and(|document| {
            document.has_focus().unwrap_or(false)
                && document.active_element().as_ref() == Some(canvas.unchecked_ref())
        })
    })
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_focus_changed() {
    WEB_PROXY.with(|proxy| {
        if let Some(proxy) = &*proxy.borrow() {
            let _ = proxy.send_event(UserEvent::BrowserFocusChanged);
        }
    });
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_modifiers_changed(shift: bool, control: bool, alt: bool, meta: bool) {
    use winit::keyboard::ModifiersState;
    let mut modifiers = ModifiersState::empty();
    modifiers.set(ModifiersState::SHIFT, shift);
    modifiers.set(ModifiersState::CONTROL, control);
    modifiers.set(ModifiersState::ALT, alt);
    modifiers.set(ModifiersState::SUPER, meta);
    WEB_PROXY.with(|proxy| {
        if let Some(proxy) = &*proxy.borrow() {
            let _ = proxy.send_event(UserEvent::BrowserModifiersChanged(modifiers));
        }
    });
}

thread_local! {
    /// The page's libraries and tutorial slots, kept so peeling can rebuild
    /// the stack from them.
    static PAGE: std::cell::RefCell<(Option<String>, Option<String>)> =
        const { std::cell::RefCell::new((None, None)) };
}

pub(crate) fn stack(
    projections: &str,
    names: bool,
) -> Result<crate::stack::Stack<crate::Editor>, String> {
    PAGE.with(|page| {
        let (libraries, slots) = &*page.borrow();
        web_embed::peeled(libraries.as_deref(), projections, slots.as_deref(), names)
    })
}

/// Draw with only some libraries' projections, keeping every definition,
/// the document, and the selection.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_projections(projections: &str, names: bool) -> Result<(), wasm_bindgen::JsValue> {
    stack(projections, names).map_err(|error| wasm_bindgen::JsValue::from_str(&error))?;
    WEB_PROXY.with(|proxy| {
        if let Some(proxy) = &*proxy.borrow() {
            let _ = proxy.send_event(UserEvent::ProjectionsChanged {
                projections: projections.to_owned(),
                names,
            });
        }
    });
    Ok(())
}

/// Tells an embedding page how tall the document view's content is, from the
/// top of the editor, whenever that changes, so the page can fit its frame.
pub(crate) fn report_content_height(regions: &[crate::placed::ViewRegion], scale: f64) {
    thread_local! {
        static REPORTED: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
    }
    let Some(height) = web_embed::content_height(regions, scale) else {
        return;
    };
    if REPORTED.with(|reported| reported.replace(Some(height))) == Some(height) {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(parent)) = window.parent() else {
        return;
    };
    if web_sys::js_sys::Object::is(&parent, &window) {
        return;
    }
    let message = web_sys::js_sys::Object::new();
    let _ = web_sys::js_sys::Reflect::set(&message, &"type".into(), &"progred:size".into());
    let _ = web_sys::js_sys::Reflect::set(&message, &"height".into(), &height.into());
    let _ = parent.post_message(&message, &window.origin());
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_theme(theme: &str) -> Result<(), wasm_bindgen::JsValue> {
    let palette = theme
        .parse::<styles::Theme>()
        .map_err(wasm_bindgen::JsValue::from_str)?
        .palette();
    WEB_PROXY.with(|proxy| {
        if let Some(proxy) = &*proxy.borrow() {
            let _ = proxy.send_event(UserEvent::PaletteChanged(palette));
        }
    });
    Ok(())
}

/// Called by the JS host on the page thread, never on the computation worker.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn computation_finished() {
    WEB_PROXY.with(|proxy| {
        if let Some(proxy) = &*proxy.borrow() {
            let _ = proxy.send_event(UserEvent::ComputationFinished);
        }
    });
}

/// A bundled example's document, named by its file without `.gid`.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn example_source(name: &str) -> Option<String> {
    crate::command::Example::ALL
        .into_iter()
        .find(|example| example.file_stem() == name)
        .map(|example| example.source().to_owned())
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start_editor(
    source: Option<String>,
    show_menu: Option<bool>,
    on_change: Option<web_sys::js_sys::Function>,
    libraries: Option<String>,
    tutorial_slots: Option<String>,
    command_is_meta: bool,
    theme: Option<String>,
) -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let (doc, binders) = gid_text::parse(source.as_deref().unwrap_or("{}"))
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error))?;
    let mut stack = web_embed::libraries(libraries.as_deref())
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error))?;
    stack.projection = web_embed::tutorial_slots(
        tutorial_slots.as_deref(),
        stack.projection,
        &stack.libraries,
    )
    .map_err(|error| wasm_bindgen::JsValue::from_str(&error))?;
    PAGE.with(|page| *page.borrow_mut() = (libraries, tutorial_slots));
    run_document(
        doc,
        binders,
        None,
        show_menu.unwrap_or(true),
        stack,
        if command_is_meta {
            puri::keyboard::CommandModifier::Meta
        } else {
            puri::keyboard::CommandModifier::Control
        },
        theme
            .as_deref()
            .unwrap_or("light")
            .parse::<styles::Theme>()
            .map_err(wasm_bindgen::JsValue::from_str)?
            .palette(),
        on_change,
    );
    Ok(())
}
