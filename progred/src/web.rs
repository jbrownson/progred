//! Browser entry points called from the embedding page.

use crate::{UserEvent, display::widget::style, gid_text, run_document, web_embed};
use wasm_bindgen::JsCast;
use winit::platform::web::WindowExtWebSys;
use winit::window::Window;

thread_local! {
    pub(crate) static WEB_PROXY: std::cell::RefCell<Option<winit::event_loop::EventLoopProxy<UserEvent>>> =
        const { std::cell::RefCell::new(None) };
}

/// Report panics on the browser's console, in the page and each worker.
pub fn install_panic_hook() {
    console_error_panic_hook::set_once();
}

/// Sends `event` into the editor's loop, once there is one.
fn send(event: UserEvent) {
    WEB_PROXY.with(|proxy| {
        if let Some(proxy) = &*proxy.borrow() {
            let _ = proxy.send_event(event);
        }
    });
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
    send(UserEvent::BrowserFocusChanged);
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_modifiers_changed(shift: bool, control: bool, alt: bool, meta: bool) {
    use winit::keyboard::ModifiersState;
    let mut modifiers = ModifiersState::empty();
    modifiers.set(ModifiersState::SHIFT, shift);
    modifiers.set(ModifiersState::CONTROL, control);
    modifiers.set(ModifiersState::ALT, alt);
    modifiers.set(ModifiersState::SUPER, meta);
    send(UserEvent::BrowserModifiersChanged(modifiers));
}

thread_local! {
    /// The page's libraries and tutorial slots, kept so peeling can rebuild
    /// the stack from them.
    static PAGE: std::cell::RefCell<(Option<String>, Option<String>)> =
        const { std::cell::RefCell::new((None, None)) };
}

/// Draw with only some libraries' projections, keeping every definition,
/// the document, and the selection.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_projections(projections: &str, names: bool) -> Result<(), wasm_bindgen::JsValue> {
    PAGE.with(|page| {
        let (libraries, slots) = &*page.borrow();
        web_embed::peeled(libraries.as_deref(), projections, slots.as_deref(), names)
    })
    .map(|stack| send(UserEvent::ProjectionsChanged(stack)))
    .map_err(|error| wasm_bindgen::JsValue::from_str(&error))
}

/// Tells an embedding page how tall the document view's content is, from the
/// top of the editor, so the page can fit its frame.
pub(crate) fn report_content_height(height: f64) {
    if let Some(window) = web_sys::window()
        && let Ok(Some(parent)) = window.parent()
        && !web_sys::js_sys::Object::is(&parent, &window)
    {
        let message = web_sys::js_sys::Object::new();
        let _ = web_sys::js_sys::Reflect::set(&message, &"type".into(), &"progred:size".into());
        let _ = web_sys::js_sys::Reflect::set(&message, &"height".into(), &height.into());
        let _ = parent.post_message(&message, &window.origin());
    }
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn set_theme(theme: &str) -> Result<(), wasm_bindgen::JsValue> {
    let palette = theme
        .parse::<style::Theme>()
        .map_err(wasm_bindgen::JsValue::from_str)?
        .palette();
    send(UserEvent::PaletteChanged(palette));
    Ok(())
}

/// Called by the JS host on the page thread, never on the computation worker.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn computation_finished() {
    send(UserEvent::ComputationFinished);
}

/// Ask what to call a saved file; `None` is a cancelled prompt. A name
/// without the extension gets it, so a bare "forest" saves as forest.gid.
pub(crate) fn ask_name(default: &str) -> Option<String> {
    web_sys::window()?
        .prompt_with_message_and_default("Save as", default)
        .ok()?
        .as_deref()
        .map(str::trim)
        .filter(|answer| !answer.is_empty())
        .map(|answer| {
            if answer.ends_with(".gid") {
                answer.to_owned()
            } else {
                format!("{answer}.gid")
            }
        })
}

/// Hand the browser a file to save: a link to the text, clicked.
pub(crate) fn download(name: &str, text: &str) -> Result<(), wasm_bindgen::JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or("no document")?;
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("text/plain");
    let parts = web_sys::js_sys::Array::of1(&text.into());
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let link: web_sys::HtmlAnchorElement = document.create_element("a")?.unchecked_into();
    link.set_href(&url);
    link.set_download(name);
    link.click();
    web_sys::Url::revoke_object_url(&url)
}

/// Offer the browser's file picker; the chosen file's text arrives as
/// [`UserEvent::Opened`] once read.
pub(crate) fn pick_document(proxy: winit::event_loop::EventLoopProxy<UserEvent>) {
    if let Some(document) = web_sys::window().and_then(|window| window.document())
        && let Ok(element) = document.create_element("input")
    {
        let input: web_sys::HtmlInputElement = element.unchecked_into();
        input.set_type("file");
        input.set_accept(".gid");
        let picked = input.clone();
        let chosen = wasm_bindgen::closure::Closure::once(move || {
            if let Some(file) = picked.files().and_then(|files| files.get(0)) {
                let name = file.name();
                wasm_bindgen_futures::spawn_local(async move {
                    match wasm_bindgen_futures::JsFuture::from(file.text()).await {
                        Ok(text) => {
                            let source = text.as_string().unwrap_or_default();
                            let _ = proxy.send_event(UserEvent::Opened { name, source });
                        }
                        Err(error) => web_sys::console::error_1(&error),
                    }
                });
            }
        });
        input.set_onchange(Some(chosen.as_ref().unchecked_ref()));
        chosen.forget();
        input.click();
    }
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
    let (doc, binders) = source
        .as_deref()
        .map(gid_text::parse)
        .transpose()
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error))?
        .unwrap_or_default();
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
            .parse::<style::Theme>()
            .map_err(wasm_bindgen::JsValue::from_str)?
            .palette(),
        on_change,
    );
    Ok(())
}
