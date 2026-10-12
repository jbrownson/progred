//! The editor driven as a person would, without a window. Pointer input and
//! modifiers arrive as Winit window events through the shell's own
//! translation, keys as the platform delivers them, and each step presents a
//! frame. Tests read the frames back through the queries in `inspect`.

use crate::EditorRunner;
use kurbo::{Point, Size, Vec2};
use puri::draw::DrawList;
use ui_events::keyboard::{Key, KeyState, KeyboardEvent, Modifiers};
use ui_events::pointer::PointerEvent;
use ui_events_winit::WindowEventTranslation;
use winit::dpi::PhysicalPosition;
use winit::event::{
    DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
};
use winit::keyboard::ModifiersState;

#[cfg(test)]
mod inspect;

/// `page` is whatever a test knows about where the editor is embedded.
pub(crate) struct Driver<P = ()> {
    pub(crate) runner: EditorRunner,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "tests read the page an embedded editor sits in")
    )]
    pub(crate) page: P,
    painted: DrawList,
    held: ModifiersState,
    scale: f64,
    viewport: Size,
}

impl Driver {
    /// Install the editor's first frame in a window of `viewport`.
    pub(crate) fn new(editor: crate::Editor, viewport: Size) -> Self {
        Self::embedded(editor, viewport, ())
    }
}

impl<P> Driver<P> {
    pub(crate) fn embedded(editor: crate::Editor, viewport: Size, page: P) -> Self {
        let mut runner = EditorRunner::new(editor);
        runner.refresh_frame(1.0, viewport);
        let mut driver = Self {
            runner,
            page,
            painted: DrawList::new(),
            held: ModifiersState::empty(),
            scale: 1.0,
            viewport,
        };
        driver.present();
        driver
    }

    pub(crate) fn editor(&self) -> &crate::Editor {
        &self.runner.editor
    }

    /// The last frame's drawing.
    pub(crate) fn painted(&self) -> &DrawList {
        &self.painted
    }

    /// The wheel turned by `lines` with the pointer at `point`: positive
    /// `y` scrolls up, positive `x` left.
    pub(crate) fn scroll_at(&mut self, point: Point, lines: Vec2) {
        self.move_to(point);
        self.window_event(WindowEvent::MouseWheel {
            device_id: DeviceId::dummy(),
            delta: MouseScrollDelta::LineDelta(lines.x as f32, lines.y as f32),
            phase: TouchPhase::Moved,
        });
    }

    pub(crate) fn present(&mut self) {
        for _ in 0..8 {
            self.runner
                .flush_before_window_event(&WindowEvent::RedrawRequested);
            let paint = self.runner.prepare_paint(self.scale, self.viewport);
            self.painted = DrawList::new();
            puri::frame::render(paint.renders, &mut self.painted);
            if !self.runner.frame_presented() {
                return;
            }
        }
        panic!("every presented frame asked for another");
    }

    fn window_event(&mut self, event: WindowEvent) {
        let pointer = self.translate(&event);
        self.dispatch(&event, pointer);
    }

    /// What the window's reducer makes of `event`, stamped with the moment it
    /// arrives.
    fn translate(&mut self, event: &WindowEvent) -> Option<PointerEvent> {
        match crate::translate_window_event(&mut self.runner.editor.reducer, self.scale, event) {
            Some(WindowEventTranslation::Pointer(pointer)) => Some(pointer),
            _ => None,
        }
    }

    fn dispatch(&mut self, event: &WindowEvent, pointer: Option<PointerEvent>) {
        self.runner.flush_before_window_event(event);
        if let WindowEvent::ModifiersChanged(modifiers) = event {
            self.runner.modifiers_changed(
                ui_events_winit::keyboard::from_winit_modifier_state(modifiers.state()),
                self.scale,
                self.viewport,
            );
        }
        if let Some(pointer) = pointer {
            self.runner
                .pointer_event(&pointer, self.scale, self.viewport);
        }
        self.present();
    }

    pub(crate) fn hold(&mut self, modifiers: ModifiersState) {
        self.held = modifiers;
        self.window_event(WindowEvent::ModifiersChanged(modifiers.into()));
    }

    pub(crate) fn move_to(&mut self, point: Point) {
        self.window_event(WindowEvent::CursorMoved {
            device_id: DeviceId::dummy(),
            position: PhysicalPosition::new(point.x, point.y),
        });
    }

    fn button(&mut self, state: ElementState) {
        self.window_event(WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state,
            button: MouseButton::Left,
        });
    }

    /// A moment passes: the next press starts a new click rather than
    /// continuing the last one.
    fn pause(&mut self) {
        self.runner.editor.reducer = Default::default();
        crate::translate_window_event(
            &mut self.runner.editor.reducer,
            self.scale,
            &WindowEvent::ModifiersChanged(self.held.into()),
        );
    }

    /// `count` presses in quick succession: two make a double click.
    pub(crate) fn clicks_at(&mut self, point: Point, count: usize) {
        self.pause();
        self.move_to(point);
        // A double click's presses arrive together: however long the editor
        // takes with the first, the second still lands within the interval.
        let presses = (0..count)
            .flat_map(|_| [ElementState::Pressed, ElementState::Released])
            .map(|state| {
                let event = WindowEvent::MouseInput {
                    device_id: DeviceId::dummy(),
                    state,
                    button: MouseButton::Left,
                };
                let pointer = self.translate(&event);
                (event, pointer)
            })
            .collect::<Vec<_>>();
        for (event, pointer) in presses {
            self.dispatch(&event, pointer);
        }
    }

    pub(crate) fn press_at(&mut self, point: Point) {
        self.pause();
        self.move_to(point);
        self.button(ElementState::Pressed);
    }

    pub(crate) fn release(&mut self) {
        self.button(ElementState::Released);
    }

    pub(crate) fn drag(&mut self, from: Point, to: &[Point]) {
        self.press_at(from);
        for point in to {
            self.move_to(*point);
        }
        self.release();
    }

    /// One key, down and up, as the page forwards it; whether the editor
    /// took the press.
    pub(crate) fn keystroke(&mut self, key: Key, modifiers: Modifiers) -> bool {
        self.runner.flush_pending_continuous();
        if self.runner.editor.modifiers != modifiers {
            self.runner
                .modifiers_changed(modifiers, self.scale, self.viewport);
        }
        let event = |state| KeyboardEvent {
            state,
            key: key.clone(),
            modifiers,
            ..Default::default()
        };
        let handled = self
            .runner
            .keyboard_event(&event(KeyState::Down), self.scale, self.viewport);
        self.runner
            .keyboard_event(&event(KeyState::Up), self.scale, self.viewport);
        if !modifiers.is_empty() {
            self.runner
                .modifiers_changed(Modifiers::empty(), self.scale, self.viewport);
        }
        self.present();
        handled
    }
}

/// The key a character is typed with, shifted for capitals and the symbols
/// that share a key with a digit or punctuation on a US layout.
pub(crate) fn keystroke_for(character: char) -> (Key, Modifiers) {
    let shifted = character.is_ascii_uppercase() || "~!@#$%^&*()_+{}|:\"<>?".contains(character);
    (
        Key::Character(character.to_string()),
        if shifted {
            Modifiers::SHIFT
        } else {
            Modifiers::empty()
        },
    )
}
