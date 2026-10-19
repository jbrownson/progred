//! The application: windows, their lifecycle, and app-level commands.

use crate::UserEvent;
use crate::command::{AppCommand, Command};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::editor::new_editor;
use crate::editor::{AfterDiscard, Editor, EditorRunner, RenderState, viewport};
use crate::frame::PendingPaint;
use crate::gid_text;
#[cfg(target_os = "macos")]
use crate::macos_surface;
#[cfg(target_os = "macos")]
use crate::macos_window;
#[cfg(target_os = "macos")]
use crate::native_menu;
#[cfg(target_arch = "wasm32")]
use crate::web::browser_editor_focused;
#[cfg(target_arch = "wasm32")]
use crate::web_embed;
#[cfg(target_arch = "wasm32")]
use crate::web_render;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::{canonical, text_dialog};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::{display::widget::style, modifiers, platform, stack, text_store};
#[cfg(not(target_arch = "wasm32"))]
use kurbo::Size;
use std::path::PathBuf;
use std::sync::Arc;
use web_time::Instant;

#[cfg(any(target_os = "macos", target_os = "linux"))]
use parley::FontContext;
use puri::handler::ImeEvent;
#[cfg(not(target_arch = "wasm32"))]
use puri_vello::compositor::SplitCanvas;
use puri_vello::compositor::{Compositor, Layer, Resources};
use ui_events_winit::WindowEventTranslation;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu::{self, CurrentSurfaceTexture};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::HtmlCanvasElement;
use winit::application::ApplicationHandler;
#[cfg(not(target_arch = "wasm32"))]
use winit::dpi::LogicalSize;
use winit::event::{Ime, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
#[cfg(target_arch = "wasm32")]
use winit::platform::web::{WindowAttributesExtWebSys, WindowExtWebSys};
#[cfg(target_os = "linux")]
use winit::platform::x11::WindowAttributesExtX11;
use winit::window::{Window, WindowId};

/// Process-wide state: the GPU, the editors, and what a new window's
/// editor starts from.
pub(crate) struct App {
    #[cfg(target_arch = "wasm32")]
    web_observer: Option<web_embed::Observer>,
    /// The document's content height the embedding page was last told.
    #[cfg(target_arch = "wasm32")]
    reported_height: Option<f64>,
    #[cfg(target_arch = "wasm32")]
    web_renderer: web_render::Renderer,
    #[cfg(not(target_arch = "wasm32"))]
    context: RenderContext,
    #[cfg(not(target_arch = "wasm32"))]
    renderers: Vec<Option<Compositor>>,
    /// The libraries, with their Rust functions and composed projection,
    /// a new window's editor starts with; each editor holds a cheap clone.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    stack: stack::Stack,
    /// The font database master; editors hold cheap clones over the
    /// same shared font data.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fonts: FontContext,
    #[cfg(target_os = "macos")]
    native_menu: native_menu::Menu,
    #[cfg(target_os = "macos")]
    appearance: Option<winit::window::Theme>,
    proxy: winit::event_loop::EventLoopProxy<UserEvent>,
    /// New windows draw the in-window menu system.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    drawn_menu: bool,
    pub(crate) editors: Vec<EditorRunner>,
    /// The window whose editor application-level commands target.
    focused: Option<WindowId>,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    quit: QuitState,
    /// AppKit's running cascade origin for windows without a saved
    /// frame.
    #[cfg(target_os = "macos")]
    cascade: macos_window::CascadePoint,
}

/// Quit reviews windows one at a time, focused first, each through its
/// discard sheet. Only declining a sheet the chain itself presented
/// abandons the quit.
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[derive(Clone, Copy, PartialEq, Eq)]
enum QuitState {
    Idle,
    Draining { awaiting: Option<WindowId> },
}

impl ApplicationHandler<UserEvent> for App {
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        for runner in &mut self.editors {
            if let RenderState::Active { window, .. } = &runner.editor.state {
                let window = window.clone();
                if runner.editor.timers.fire_due(now) {
                    if !runner.flush_pending_continuous() {
                        runner.refresh_frame(runner.editor.scale(&window), viewport(&window));
                    }
                    runner.sync_window(&window);
                    window.request_redraw();
                }
            }
        }
        event_loop.set_control_flow(
            self.editors
                .iter()
                .filter(|runner| matches!(runner.editor.state, RenderState::Active { .. }))
                .filter_map(|runner| runner.editor.timers.deadline())
                .min()
                .map_or(ControlFlow::Wait, ControlFlow::WaitUntil),
        );
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        for runner in &mut self.editors {
            if runner.flush_pending_continuous()
                && let RenderState::Active { window, .. } = &runner.editor.state
            {
                window.request_redraw();
            }
        }
        match event {
            #[cfg(target_arch = "wasm32")]
            UserEvent::BrowserModifiersChanged(modifiers) => {
                if !self.editors.is_empty() {
                    self.editor_window_event(
                        event_loop,
                        0,
                        WindowEvent::ModifiersChanged(modifiers.into()),
                    );
                }
            }
            #[cfg(target_arch = "wasm32")]
            UserEvent::PaletteChanged(palette) => {
                for runner in &mut self.editors {
                    change_palette(runner, palette);
                }
            }
            #[cfg(target_arch = "wasm32")]
            UserEvent::ProjectionsChanged(stack) => {
                for runner in &mut self.editors {
                    if let RenderState::Active { window, .. } = &runner.editor.state {
                        let window = window.clone();
                        runner.stack_changed(
                            stack.clone(),
                            runner.editor.scale(&window),
                            viewport(&window),
                        );
                        window.request_redraw();
                    } else {
                        runner.editor.replace_stack(stack.clone());
                    }
                }
            }
            #[cfg(target_arch = "wasm32")]
            UserEvent::BrowserFocusChanged => {
                if let Some(runner) = self.editors.first_mut()
                    && let RenderState::Active { window, .. } = &runner.editor.state
                {
                    let window = window.clone();
                    if runner.focus_changed(
                        browser_editor_focused(&window),
                        runner.editor.scale(&window),
                        viewport(&window),
                    ) {
                        window.request_redraw();
                    }
                }
            }
            UserEvent::ComputationFinished => {
                for runner in &mut self.editors {
                    if runner.editor.computations.poll()
                        && let RenderState::Active { window, .. } = &runner.editor.state
                    {
                        let window = window.clone();
                        runner.refresh_frame(runner.editor.scale(&window), viewport(&window));
                        runner.sync_window(&window);
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
            #[cfg(target_arch = "wasm32")]
            UserEvent::Opened { name, source } => match gid_text::parse(&source) {
                Ok((doc, binders)) => {
                    if let Some(index) = self.focused_index() {
                        self.editors[index].adopt_model(doc, Some(PathBuf::from(name)), binders);
                    }
                }
                Err(error) => {
                    web_sys::console::error_1(&format!("failed to open {name}: {error}").into())
                }
            },
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            UserEvent::Discard { window, accepted } => {
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
                if let Some(index) = self.editor_index(window) {
                    let pending = self.editors[index].editor.pending_discard.take();
                    if accepted && let Some(then) = pending {
                        self.proceed(event_loop, index, then);
                    }
                }
            }
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // After launch, so winit cannot replace it (its own default
        // menu is disabled at loop construction).
        #[cfg(target_os = "macos")]
        {
            self.native_menu.install();
            self.native_menu.sync(None, self.appearance);
        }

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
        let focused = matches!(event, WindowEvent::Focused(true));
        if focused {
            self.focused = Some(window_id);
        }
        if let Some(index) = self.editor_index(window_id) {
            if focused {
                self.sync_menus(index);
            }
            self.editor_window_event(event_loop, index, event);
        }
    }
}

impl App {
    /// The app before any window opens, `editor` to be its first window's.
    /// A new desktop window's editor starts from `stack` and `fonts`, and
    /// draws its own menu when `drawn_menu`.
    pub(crate) fn new(
        proxy: winit::event_loop::EventLoopProxy<UserEvent>,
        editor: Editor,
        #[cfg(any(target_os = "macos", target_os = "linux"))] stack: stack::Stack,
        #[cfg(any(target_os = "macos", target_os = "linux"))] fonts: FontContext,
        #[cfg(any(target_os = "macos", target_os = "linux"))] drawn_menu: bool,
        #[cfg(target_arch = "wasm32")] on_change: Option<web_sys::js_sys::Function>,
    ) -> Self {
        Self {
            #[cfg(target_arch = "wasm32")]
            web_observer: on_change.map(web_embed::Observer::new),
            #[cfg(target_arch = "wasm32")]
            reported_height: None,
            #[cfg(target_arch = "wasm32")]
            web_renderer: web_render::take(),
            #[cfg(not(target_arch = "wasm32"))]
            context: RenderContext::new(),
            #[cfg(not(target_arch = "wasm32"))]
            renderers: vec![],
            #[cfg(target_os = "macos")]
            native_menu: native_menu::Menu::new(proxy.clone(), stack.switchable()),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            stack,
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            fonts,
            #[cfg(target_os = "macos")]
            appearance: None,
            proxy,
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            drawn_menu,
            editors: vec![EditorRunner::new(editor)],
            focused: None,
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            quit: QuitState::Idle,
            #[cfg(target_os = "macos")]
            cascade: macos_window::initial_cascade(),
        }
    }

    fn editor_index(&self, id: WindowId) -> Option<usize> {
        self.editors
            .iter()
            .position(|runner| runner.editor.window_id() == Some(id))
    }

    /// The editor application-level commands act on: the focused
    /// window's, or the first one's while focus is unknown.
    fn focused_index(&self) -> Option<usize> {
        self.focused
            .and_then(|id| self.editor_index(id))
            .or_else(|| (!self.editors.is_empty()).then_some(0))
    }

    /// Opens a document in a new window; each window holds one document.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn open_editor(
        &mut self,
        event_loop: &ActiveEventLoop,
        doc: gid::Document,
        path: Option<PathBuf>,
        binders: gid_text::Binders,
    ) {
        self.editors.push(EditorRunner::new(new_editor(
            style::Theme::Light.palette(),
            modifiers::native(),
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
            self.native_menu.sync(None, self.appearance);
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
        if self.quit == (QuitState::Draining { awaiting: None }) {
            match self.focused_index() {
                None => event_loop.exit(),
                Some(index) if self.editors[index].editor.model.dirty() => {
                    if let RenderState::Active { window, .. } = &self.editors[index].editor.state {
                        window.focus_window();
                        self.quit = QuitState::Draining {
                            awaiting: Some(window.id()),
                        };
                    }
                    self.request_discard(event_loop, index, AfterDiscard::CloseWindow);
                }
                Some(index) => {
                    self.close_editor(event_loop, index);
                    self.advance_quit(event_loop);
                }
            }
        }
    }

    /// Gives suspended editor `index` its window back, or its first one,
    /// and draws it.
    fn resume_editor(&mut self, event_loop: &ActiveEventLoop, index: usize) {
        if let RenderState::Suspended(cached) = &mut self.editors[index].editor.state {
            let cached = cached.take();
            let window = cached.unwrap_or_else(|| self.create_window(event_loop, index));
            #[cfg(target_os = "macos")]
            {
                window.set_theme(self.appearance);
                self.editors[index].editor.palette =
                    macos_window::palette(self.appearance, window.theme());
            }
            let state = self.activate(window.clone());
            let runner = &mut self.editors[index];
            runner.editor.state = state;
            #[cfg(not(target_arch = "wasm32"))]
            {
                runner.editor.focused = window.has_focus();
            }
            #[cfg(target_arch = "wasm32")]
            {
                runner.editor.focused = browser_editor_focused(&window);
            }
            runner.refresh_frame(runner.editor.scale(&window), viewport(&window));
            runner.sync_window(&window);
            window.request_redraw();
        }
    }

    /// A new window for editor `index`'s document.
    fn create_window(&mut self, event_loop: &ActiveEventLoop, index: usize) -> Arc<Window> {
        let editor = &self.editors[index].editor;
        let attributes = Window::default_attributes().with_title(editor.title());
        #[cfg(target_os = "macos")]
        let attributes = attributes.with_theme(self.appearance);
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
                .with_active(false)
                .with_prevent_default(true)
        };
        let window = event_loop.create_window(attributes).unwrap();
        #[cfg(target_os = "macos")]
        {
            macos_window::place_and_autosave_frame(
                &window,
                editor.doc_path.as_deref(),
                &mut self.cascade,
            );
            macos_window::set_represented(&window, editor.doc_path.as_deref());
        }
        Arc::new(window)
    }

    /// `window`'s render state once it can draw: a GPU surface, with a
    /// compositor for the surface's device.
    #[cfg(not(target_arch = "wasm32"))]
    fn activate(&mut self, window: Arc<Window>) -> RenderState {
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
            Compositor::new(
                &self.context.devices[surface.dev_id].device,
                &self.context.devices[surface.dev_id].queue,
            )
            .expect("Couldn't create renderer")
        });
        RenderState::Active {
            surface: Box::new(surface),
            valid_surface: true,
            window,
        }
    }

    /// `window`'s render state once it can draw: its canvas.
    #[cfg(target_arch = "wasm32")]
    fn activate(&mut self, window: Arc<Window>) -> RenderState {
        RenderState::Active {
            canvas: window.canvas().expect("Winit web canvas"),
            window,
        }
    }

    fn editor_window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        index: usize,
        event: WindowEvent,
    ) {
        if let RenderState::Active { window, .. } = &self.editors[index].editor.state {
            let window = window.clone();
            let runner = &mut self.editors[index];
            let scale = runner.editor.scale(&window);
            let viewport = viewport(&window);
            // Preserve motion samples without minting intermediate frames.
            // Discrete input (including release/cancel) first settles the batch.
            if runner.flush_before_window_event(&event) {
                window.request_redraw();
            }

            #[cfg(target_os = "macos")]
            if matches!(event, WindowEvent::ThemeChanged(_)) {
                change_palette(
                    runner,
                    macos_window::palette(self.appearance, window.theme()),
                );
            }

            if let WindowEvent::ModifiersChanged(state) = &event {
                runner.modifiers_changed(
                    ui_events_winit::keyboard::from_winit_modifier_state(state.state()),
                    scale,
                    viewport,
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
                let translation = runner.editor.reducer.reduce(scale, &event);
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
                WindowEvent::Focused(_focused) => {
                    #[cfg(not(target_arch = "wasm32"))]
                    let focused = _focused;
                    #[cfg(target_arch = "wasm32")]
                    let focused = browser_editor_focused(&window);
                    if self.editors[index].focus_changed(focused, scale, viewport) {
                        window.request_redraw();
                    }
                }

                WindowEvent::CloseRequested => {
                    #[cfg(any(target_os = "macos", target_os = "linux"))]
                    self.request_discard(event_loop, index, AfterDiscard::CloseWindow);
                    #[cfg(target_arch = "wasm32")]
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
                    if self.editors[index].window_moved(scale, viewport) {
                        window.request_redraw();
                    }
                }

                WindowEvent::RedrawRequested => self.redraw(index),
                _ => {}
            }
        }
    }
}

/// Draws `runner`'s editor in `palette` from now on: an active window
/// repaints now, a suspended one when it resumes.
#[cfg(any(target_os = "macos", target_arch = "wasm32"))]
fn change_palette(runner: &mut EditorRunner, palette: crate::display::widget::style::Palette) {
    match &runner.editor.state {
        RenderState::Active { window, .. } => {
            let window = window.clone();
            if runner.palette_changed(palette, runner.editor.scale(&window), viewport(&window)) {
                window.request_redraw();
            }
        }
        RenderState::Suspended(_) => runner.editor.palette = palette,
    }
}

impl App {
    /// Menu enablement follows the model: gray what can't act. Save
    /// stays live for untitled documents — it defers to the save
    /// panel, per platform convention.
    fn sync_menus(&self, index: usize) {
        #[cfg(target_os = "macos")]
        {
            let editor = &self.editors[index].editor;
            self.native_menu.sync(
                Some((editor.menu_availability(), editor.menu_toggles())),
                self.appearance,
            );
        }
        #[cfg(not(target_os = "macos"))]
        let _ = index;
    }

    /// Application commands need no window; document commands act on
    /// the focused window's editor (the native menu's routing — the
    /// drawn menu dispatches document commands to its own editor
    /// directly).
    fn run_command(&mut self, event_loop: &ActiveEventLoop, command: Command) {
        match command {
            Command::App(command) => self.run_app_command(event_loop, command),
            Command::Doc(command) => {
                if let Some(runner) = self.focused_index().map(|index| &mut self.editors[index])
                    && let Some(window) = runner.editor.window()
                {
                    let geometry = runner.frame.dispatch.geometry(runner.editor.scale(&window));
                    runner.editor.run_doc_command(command, geometry);
                    // After the command, which may have zoomed.
                    runner.refresh_frame(runner.editor.scale(&window), viewport(&window));
                    window.request_redraw();
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
                gid::Document::default(),
                gid_text::Binders::new(),
            ),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            AppCommand::NewWindow => self.open_editor(
                event_loop,
                gid::Document::default(),
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
            #[cfg(target_arch = "wasm32")]
            AppCommand::Open => {
                if let Some(index) = self.focused_index() {
                    self.request_discard(event_loop, index, AfterDiscard::Open);
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
            AppCommand::Example(example) => match gid_text::parse(example.source()) {
                Ok((doc, binders)) => self.new_document(event_loop, doc, binders),
                Err(error) => panic!("built-in example failed to parse: {error}"),
            },
            #[cfg(target_os = "macos")]
            AppCommand::Appearance(appearance) => {
                self.appearance = appearance;
                for runner in &mut self.editors {
                    if let Some(window) = runner.editor.window() {
                        window.set_theme(appearance);
                        change_palette(runner, macos_window::palette(appearance, window.theme()));
                    }
                }
                if let Some(index) = self.focused_index() {
                    self.sync_menus(index);
                } else {
                    self.native_menu.sync(None, appearance);
                }
            }
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

    /// Gate an action on unsaved changes: a clean document proceeds
    /// immediately; a dirty one asks first, in the browser through its
    /// confirm dialog, and on the desktop through the window's sheet,
    /// continuing through a [`UserEvent::Discard`] when it resolves.
    fn request_discard(&mut self, event_loop: &ActiveEventLoop, index: usize, then: AfterDiscard) {
        let editor = &self.editors[index].editor;
        match (editor.pending_discard.is_some(), editor.model.dirty()) {
            // Already asking.
            (true, _) => {}
            (false, false) => self.proceed(event_loop, index, then),
            (false, true) => {
                #[cfg(target_arch = "wasm32")]
                self.confirm_discard(event_loop, index, then);
                #[cfg(any(target_os = "macos", target_os = "linux"))]
                self.show_discard_sheet(index, then);
            }
        }
    }

    /// Ask through the browser's confirm dialog, proceeding if accepted.
    #[cfg(target_arch = "wasm32")]
    fn confirm_discard(&mut self, event_loop: &ActiveEventLoop, index: usize, then: AfterDiscard) {
        if web_sys::window()
            .and_then(|window| window.confirm_with_message("Discard unsaved changes?").ok())
            .unwrap_or(false)
        {
            self.proceed(event_loop, index, then);
        }
    }

    /// Ask through the window's sheet. Async is the one rfd path that
    /// presents natively on macOS (begin-sheet with a completion): the sync
    /// API falls back, parentless, to a CFUserNotification panel that
    /// arrives unfocused and warns on the console, and, parented, presents
    /// twice, as a sheet and a modal loop. The sheet's completion lands on
    /// the main run loop, so the answer is awaited on a throwaway thread and
    /// routed back through the proxy; blocking here would deadlock the loop
    /// the sheet needs.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn show_discard_sheet(&mut self, index: usize, then: AfterDiscard) {
        let editor = &mut self.editors[index].editor;
        if let RenderState::Active { window, .. } = &editor.state {
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
    }

    /// The action a confirmed (or unneeded) discard proceeds to.
    fn proceed(&mut self, event_loop: &ActiveEventLoop, index: usize, then: AfterDiscard) {
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
            #[cfg(target_arch = "wasm32")]
            AfterDiscard::Open => crate::web::pick_document(self.proxy.clone()),
            #[cfg(target_arch = "wasm32")]
            AfterDiscard::Quit => event_loop.exit(),
        }
    }

    /// Renders the current model to the surface, from `RedrawRequested`.
    #[cfg(not(target_arch = "wasm32"))]
    fn redraw(&mut self, index: usize) {
        // The menu bar mirrors the focused window's editor only; an
        // unfocused window's redraw must not relabel it.
        if self.focused_index() == Some(index) {
            self.sync_menus(index);
        }
        let runner = &mut self.editors[index];
        let drawable = match &runner.editor.state {
            RenderState::Active {
                surface,
                valid_surface: true,
                window,
            } => Some((
                window.clone(),
                Size::new(surface.config.width.into(), surface.config.height.into()),
            )),
            _ => None,
        };
        if let Some((window, viewport)) = drawable {
            let PendingPaint { renders, .. } =
                runner.prepare_paint(runner.editor.scale(&window), viewport);
            runner.sync_window(&window);
            let mut paint = SplitCanvas::default();
            puri::frame::render(renders, &mut paint);
            if let RenderState::Active { surface, .. } = &runner.editor.state {
                let presented = present(
                    &self.context,
                    surface,
                    self.renderers[surface.dev_id]
                        .as_mut()
                        .expect("a surface's device has a compositor"),
                    &mut runner.editor.paint_resources,
                    &paint.finish(),
                    runner.editor.palette.paper,
                )
                .expect("failed to present");
                if presented {
                    self.context.devices[surface.dev_id]
                        .device
                        .poll(wgpu::PollType::Poll)
                        .unwrap();
                }
                if !presented || runner.frame_presented() {
                    window.request_redraw();
                }
            }
        }
    }

    /// Browser presentation uses the same GPU compositor as native when
    /// available. Only setup/presentation differs; widgets remain unchanged.
    #[cfg(target_arch = "wasm32")]
    fn redraw(&mut self, index: usize) {
        if self.focused_index() == Some(index) {
            self.sync_menus(index);
        }
        let runner = &mut self.editors[index];
        if let RenderState::Active { canvas, window } = &runner.editor.state {
            let (canvas, window) = (canvas.clone(), window.clone());
            let size = window.inner_size();
            if size.width != 0 && size.height != 0 {
                if canvas.width() != size.width {
                    canvas.set_width(size.width);
                }
                if canvas.height() != size.height {
                    canvas.set_height(size.height);
                }
                let scale = runner.editor.scale(&window);
                let PendingPaint { renders, .. } = runner.prepare_paint(scale, viewport(&window));
                runner.sync_window(&window);
                let presented = self
                    .web_renderer
                    .render(
                        size.width,
                        size.height,
                        runner.editor.palette.paper,
                        |canvas| puri::frame::render(renders, canvas),
                    )
                    .expect("browser render failed");
                if let Some(observer) = &mut self.web_observer {
                    observer.notify(&runner.editor);
                }
                if let Some(height) =
                    web_embed::content_height(&runner.frame.dispatch.view_regions, scale)
                        .filter(|height| self.reported_height != Some(*height))
                {
                    self.reported_height = Some(height);
                    crate::web::report_content_height(height);
                }
                if !presented || runner.frame_presented() {
                    window.request_redraw();
                }
            }
        }
    }
}

/// Composites `layers` over `background` and presents them on `surface`:
/// false when the surface wasn't ready, so the caller draws again.
pub(crate) fn present(
    context: &RenderContext,
    surface: &RenderSurface<'_>,
    compositor: &mut Compositor,
    resources: &mut Resources,
    layers: &[Layer],
    background: peniko::Color,
) -> Result<bool, String> {
    let device = &context.devices[surface.dev_id];
    compositor
        .render(
            &device.device,
            &device.queue,
            layers,
            resources,
            &surface.target_texture,
            background,
        )
        .map_err(|error| error.to_string())
        .and_then(|output| match surface.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(texture) => {
                let mut encoder =
                    device
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Surface Blit"),
                        });
                surface.blitter.copy(
                    &device.device,
                    &mut encoder,
                    &output.texture.create_view(&Default::default()),
                    &texture.texture.create_view(&Default::default()),
                );
                device.queue.submit([encoder.finish()]);
                device.queue.present(texture);
                Ok(true)
            }
            CurrentSurfaceTexture::Outdated | CurrentSurfaceTexture::Suboptimal(_) => {
                context.configure_surface(surface);
                Ok(false)
            }
            CurrentSurfaceTexture::Occluded | CurrentSurfaceTexture::Timeout => Ok(false),
            CurrentSurfaceTexture::Lost => Err("the surface was lost".into()),
            CurrentSurfaceTexture::Validation => Err("the surface failed validation".into()),
        })
}
