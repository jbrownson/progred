//! Browser keys need an answer during their DOM callback. The web event loop
//! and this callback borrow the same app; neither retains a second editor.
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use ui_events::keyboard::{KeyState, KeyboardEvent, Location, Modifiers};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::web::EventLoopExtWebSys;
use winit::window::WindowId;

use crate::{App, UserEvent};

thread_local! {
    static APP: RefCell<Weak<RefCell<App>>> = const { RefCell::new(Weak::new()) };
}

pub(crate) fn spawn(app: App, event_loop: EventLoop<UserEvent>) {
    let app = Rc::new(RefCell::new(app));
    APP.with(|slot| *slot.borrow_mut() = Rc::downgrade(&app));
    event_loop.spawn_app(SharedApp(app));
}

struct SharedApp(Rc<RefCell<App>>);

impl ApplicationHandler<UserEvent> for SharedApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.0.borrow_mut().resumed(event_loop);
    }

    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        self.0.borrow_mut().suspended(event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        self.0.borrow_mut().window_event(event_loop, id, event);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        self.0.borrow_mut().user_event(event_loop, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.0.borrow_mut().about_to_wait(event_loop);
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        APP.with(|slot| *slot.borrow_mut() = Weak::new());
        self.0.borrow_mut().exiting(event_loop);
    }
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_keyboard(event: web_sys::KeyboardEvent) -> bool {
    let mut modifiers = Modifiers::empty();
    for (name, flag) in [
        ("Shift", Modifiers::SHIFT),
        ("Control", Modifiers::CONTROL),
        ("Alt", Modifiers::ALT),
        ("Meta", Modifiers::META),
        ("AltGraph", Modifiers::ALT_GRAPH),
        ("CapsLock", Modifiers::CAPS_LOCK),
        ("NumLock", Modifiers::NUM_LOCK),
        ("ScrollLock", Modifiers::SCROLL_LOCK),
    ] {
        modifiers.set(flag, event.get_modifier_state(name));
    }
    let key = KeyboardEvent {
        state: if event.type_() == "keyup" {
            KeyState::Up
        } else {
            KeyState::Down
        },
        key: event.key().parse().unwrap_or_default(),
        code: event.code().parse().unwrap_or_default(),
        location: match event.location() {
            1 => Location::Left,
            2 => Location::Right,
            3 => Location::Numpad,
            _ => Location::Standard,
        },
        modifiers,
        repeat: event.repeat(),
        is_composing: event.is_composing(),
    };
    APP.with(|slot| {
        let Some(app) = slot.borrow().upgrade() else {
            return false;
        };
        // A synthetic reentrant event cannot borrow the editor. Leave it to
        // the browser instead of queuing an edit after its default action.
        let Ok(mut app) = app.try_borrow_mut() else {
            return false;
        };
        let Some(runner) = app.editors.first_mut() else {
            return false;
        };
        let Some(window) = runner.editor.window() else {
            return false;
        };
        let scale = window.scale_factor();
        let size = window.inner_size();
        let viewport = kurbo::Size::new(size.width as f64, size.height as f64);
        let flushed = runner.flush_pending_continuous();
        let focus_changed =
            runner.focus_changed(crate::browser_editor_focused(&window), scale, viewport);
        let modifiers_changed = runner.editor.modifiers != modifiers;
        if modifiers_changed {
            runner.modifiers_changed(modifiers, scale, viewport);
        }
        // IME-owned keys must not execute shortcuts or insert preedit text.
        let handled = !key.is_composing && runner.keyboard_event(&key, scale, viewport);
        if handled || flushed || focus_changed || modifiers_changed {
            runner.sync_cursor(&window);
            window.request_redraw();
        }
        handled
    })
}
