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
    stack.projection = web_embed::tutorial_slots(tutorial_slots.as_deref(), stack.projection)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error))?;
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
