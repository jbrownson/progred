//! The application: windows, their lifecycle, and app-level commands.

use crate::command::{AppCommand, Command};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::editor::new_editor;
use crate::editor::{AfterDiscard, Editor, EditorRunner, RenderState};
#[cfg(not(target_arch = "wasm32"))]
use crate::frame::Paint;
use crate::input::PendingPaint;
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
use crate::{UserEvent, translate_window_event};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::{canonical, text_dialog};
use crate::{gid_text, stack};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::{modifiers, platform, styles, text_store};
use kurbo::Size;
use std::path::PathBuf;
use std::sync::Arc;
use web_time::Instant;

use parley::FontContext;
use puri::handler::ImeEvent;
#[cfg(not(target_arch = "wasm32"))]
use puri_vello::compositor::{Compositor, Resources};
use ui_events_winit::WindowEventTranslation;
#[cfg(not(target_arch = "wasm32"))]
use vello::util::RenderContext;
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
use winit::event_loop::{ActiveEventLoop, ControlFlow};
#[cfg(target_arch = "wasm32")]
use winit::platform::web::{WindowAttributesExtWebSys, WindowExtWebSys};
#[cfg(target_os = "linux")]
use winit::platform::x11::WindowAttributesExtX11;
use winit::window::{Window, WindowId};

/// Process-wide state: the GPU, the shared caches, and the editors.
pub(crate) struct App {
    #[cfg(target_arch = "wasm32")]
    pub(crate) web_observer: Option<web_embed::Observer>,
    #[cfg(target_arch = "wasm32")]
    pub(crate) web_renderer: web_render::Renderer,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) context: RenderContext,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) renderers: Vec<Option<Compositor>>,
    /// Editor configuration shared by every document loaded into the
    /// app: library cells, Rust functions, and composed projection.
    /// Each editor holds its own (cheap) clone, so a window can later
    /// filter or extend its libraries independently; this master copy
    /// seeds new editors.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) stack: stack::Stack<Editor>,
    /// The font database master; editors hold cheap clones over the
    /// same shared font data.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) fonts: FontContext,
    #[cfg(target_os = "macos")]
    pub(crate) native_menu: native_menu::Menu,
    #[cfg(target_os = "macos")]
    pub(crate) appearance: Option<winit::window::Theme>,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) proxy: winit::event_loop::EventLoopProxy<UserEvent>,
    /// New windows draw the in-window menu system.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
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

impl ApplicationHandler<UserEvent> for App {
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        for runner in &mut self.editors {
            if let RenderState::Active { window, .. } = &runner.editor.state {
                let window = window.clone();
                let size = window.inner_size();
                if runner.editor.timers.fire_due(now) {
                    if !runner.flush_pending_continuous() {
                        runner.refresh_frame(
                            runner.editor.scale(&window),
                            Size::new(size.width as f64, size.height as f64),
                        );
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
                if let Some(id) = self
                    .editors
                    .first()
                    .and_then(|runner| runner.editor.window_id())
                {
                    self.editor_window_event(
                        event_loop,
                        0,
                        id,
                        WindowEvent::ModifiersChanged(modifiers.into()),
                    );
                }
            }
            #[cfg(target_arch = "wasm32")]
            UserEvent::PaletteChanged(palette) => {
                for runner in &mut self.editors {
                    if let RenderState::Active { window, .. } = &runner.editor.state {
                        let window = window.clone();
                        let size = window.inner_size();
                        if runner.palette_changed(
                            palette,
                            runner.editor.scale(&window),
                            Size::new(size.width as f64, size.height as f64),
                        ) {
                            window.request_redraw();
                        }
                    } else {
                        runner.editor.palette = palette;
                    }
                }
            }
            #[cfg(target_arch = "wasm32")]
            UserEvent::ProjectionsChanged { projections, names } => {
                match crate::web::stack(&projections, names) {
                    Ok(stack) => {
                        for runner in &mut self.editors {
                            if let RenderState::Active { window, .. } = &runner.editor.state {
                                let window = window.clone();
                                let size = window.inner_size();
                                runner.stack_changed(
                                    stack.clone(),
                                    runner.editor.scale(&window),
                                    Size::new(size.width as f64, size.height as f64),
                                );
                                window.request_redraw();
                            } else {
                                runner.editor.replace_stack(stack.clone());
                            }
                        }
                    }
                    Err(error) => web_sys::console::error_1(&error.into()),
                }
            }
            #[cfg(target_arch = "wasm32")]
            UserEvent::BrowserFocusChanged => {
                if let Some(runner) = self.editors.first_mut()
                    && let RenderState::Active { window, .. } = &runner.editor.state
                {
                    let window = window.clone();
                    let size = window.inner_size();
                    if runner.focus_changed(
                        browser_editor_focused(&window),
                        runner.editor.scale(&window),
                        Size::new(size.width as f64, size.height as f64),
                    ) {
                        window.request_redraw();
                    }
                }
            }
            UserEvent::ComputationFinished => {
                for runner in &mut self.editors {
                    if runner.editor.computations.tasks.poll()
                        && let RenderState::Active { window, .. } = &runner.editor.state
                    {
                        let window = window.clone();
                        let size = window.inner_size();
                        runner.refresh_frame(
                            runner.editor.scale(&window),
                            Size::new(f64::from(size.width), f64::from(size.height)),
                        );
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
            styles::Theme::Light.palette(),
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
        let appearance = self.appearance;
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
            #[cfg(target_os = "macos")]
            let attributes = attributes.with_theme(appearance);
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
                    runner.editor.doc_path.as_deref(),
                    cascade,
                );
                macos_window::set_represented(&window, runner.editor.doc_path.as_deref());
            }
            Arc::new(window)
        });

        #[cfg(target_os = "macos")]
        {
            window.set_theme(appearance);
            runner.editor.palette = macos_window::palette(appearance, window.theme());
        }

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
            #[cfg(not(target_arch = "wasm32"))]
            {
                runner.editor.focused = window.has_focus();
            }
            #[cfg(target_arch = "wasm32")]
            {
                runner.editor.focused = browser_editor_focused(&window);
            }
            let size = window.inner_size();
            runner.refresh_frame(
                runner.editor.scale(&window),
                Size::new(size.width as f64, size.height as f64),
            );
            runner.sync_window(&window);
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
        let scale = runner.editor.scale(&window);
        // Preserve motion samples without minting intermediate frames.
        // Discrete input (including release/cancel) first settles the batch.
        if runner.flush_before_window_event(&event) {
            window.request_redraw();
        }

        #[cfg(target_os = "macos")]
        if matches!(event, WindowEvent::ThemeChanged(_)) {
            let size = window.inner_size();
            if runner.palette_changed(
                macos_window::palette(self.appearance, window.theme()),
                scale,
                Size::new(size.width as f64, size.height as f64),
            ) {
                window.request_redraw();
            }
        }

        if let WindowEvent::Focused(_focused) = event {
            #[cfg(not(target_arch = "wasm32"))]
            let focused = _focused;
            #[cfg(target_arch = "wasm32")]
            let focused = browser_editor_focused(&window);
            let size = window.inner_size();
            if runner.focus_changed(
                focused,
                scale,
                Size::new(size.width as f64, size.height as f64),
            ) {
                window.request_redraw();
            }
            return;
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

impl App {
    /// Menu enablement follows the model: gray what can't act. Save
    /// stays live for untitled documents — it defers to the save
    /// panel, per platform convention.
    pub(crate) fn sync_menus(&self, index: usize) {
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
    pub(crate) fn run_command(&mut self, event_loop: &ActiveEventLoop, command: Command) {
        match command {
            Command::App(command) => self.run_app_command(event_loop, command),
            Command::Doc(command) => {
                if let Some(index) = self.focused_index() {
                    let runner = &mut self.editors[index];
                    if let Some(window) = runner.editor.window() {
                        let size = window.inner_size();
                        let geometry = runner.frame.dispatch.geometry(runner.editor.scale(&window));
                        runner.editor.run_doc_command(command, geometry);
                        // After the command, which may have zoomed.
                        runner.refresh_frame(
                            runner.editor.scale(&window),
                            Size::new(size.width as f64, size.height as f64),
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
                        let palette = macos_window::palette(appearance, window.theme());
                        if matches!(runner.editor.state, RenderState::Active { .. }) {
                            let size = window.inner_size();
                            if runner.palette_changed(
                                palette,
                                runner.editor.scale(&window),
                                Size::new(size.width as f64, size.height as f64),
                            ) {
                                window.request_redraw();
                            }
                        } else {
                            runner.editor.palette = palette;
                        }
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
            #[cfg(target_arch = "wasm32")]
            AfterDiscard::Open => crate::web::pick_document(self.proxy.clone()),
            #[cfg(target_arch = "wasm32")]
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
        let scale = runner.editor.scale(&window);
        let width = surface.config.width;
        let height = surface.config.height;

        let viewport = Size::new(width as f64, height as f64);
        let PendingPaint { renders, .. } = runner.prepare_paint(scale, viewport);
        runner.sync_window(&window);
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
                runner.editor.palette.paper,
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
        let scale = runner.editor.scale(&window);
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
        runner.sync_window(&window);
        let presented = self
            .web_renderer
            .render(width, height, runner.editor.palette.paper, |canvas| {
                puri::frame::render(renders, canvas)
            })
            .expect("browser render failed");
        if let Some(observer) = &mut self.web_observer {
            observer.notify(&runner.editor);
        }
        if !presented || runner.frame_presented() {
            window.request_redraw();
        }
    }
}
