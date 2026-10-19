use crate::frame::DispatchContext;
use crate::{Editor, EditorRunner, navigate};
use kurbo::{Point, Rect, Size};
use puri::handler::{Event, Handler, ImeEvent, NavigationDirection};
use std::borrow::Cow;
use ui_events::ScrollDelta;
use ui_events::keyboard::{KeyboardEvent, Modifiers};
use ui_events::pointer::{
    PointerEvent, PointerGestureEvent, PointerId, PointerInfo, PointerScrollEvent, PointerType,
    PointerUpdate,
};
use ui_events_winit::WindowEventReducer;
use winit::event::WindowEvent;

pub(crate) type PendingScroll = PendingBatch<PointerScrollEvent>;
pub(crate) type PendingGesture = PendingBatch<PointerGestureEvent>;

pub(crate) struct PendingBatch<T> {
    pub(crate) events: Vec<T>,
    pub(crate) scale: f64,
    pub(crate) viewport: Size,
}

pub(crate) struct PendingPointer {
    pub(crate) event: PointerUpdate,
    pub(crate) start: Point,
    pub(crate) scale: f64,
    pub(crate) viewport: Size,
}

impl PendingPointer {
    pub(crate) fn merge(&mut self, mut next: Self) -> Result<(), Self> {
        if self.scale == next.scale
            && self.viewport == next.viewport
            && self.event.pointer == next.event.pointer
            && self.event.current.buttons == next.event.current.buttons
            && self.event.current.modifiers == next.event.current.modifiers
        {
            self.event.coalesced.push(std::mem::replace(
                &mut self.event.current,
                next.event.current,
            ));
            self.event.coalesced.append(&mut next.event.coalesced);
            self.event.predicted = next.event.predicted;
            Ok(())
        } else {
            Err(next)
        }
    }
}

pub(crate) fn continuous_input(event: &WindowEvent) -> bool {
    matches!(
        event,
        WindowEvent::MouseWheel { .. }
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::PinchGesture {
                phase: winit::event::TouchPhase::Moved,
                ..
            }
            | WindowEvent::RotationGesture {
                phase: winit::event::TouchPhase::Moved,
                ..
            }
            | WindowEvent::Touch(winit::event::Touch {
                phase: winit::event::TouchPhase::Moved,
                ..
            })
    )
}

impl<T> PendingBatch<T> {
    pub(crate) fn merge(&mut self, mut next: Self) -> Result<(), Self> {
        if self.scale == next.scale && self.viewport == next.viewport {
            self.events.append(&mut next.events);
            Ok(())
        } else {
            Err(next)
        }
    }
}

fn pointer_position(event: &PointerEvent) -> Option<Point> {
    match event {
        PointerEvent::Down(e) | PointerEvent::Up(e) => {
            Some(Point::new(e.state.position.x, e.state.position.y))
        }
        PointerEvent::Move(u) => Some(Point::new(u.current.position.x, u.current.position.y)),
        PointerEvent::Scroll(e) => Some(Point::new(e.state.position.x, e.state.position.y)),
        PointerEvent::Gesture(e) => Some(Point::new(e.state.position.x, e.state.position.y)),
        _ => None,
    }
}

fn window_pointer(position: Point, size: Size) -> Option<Point> {
    Rect::from_origin_size(Point::ZERO, size)
        .contains(position)
        .then_some(position)
}

/// The editor's structural keys, beneath every placed handler. An arrow
/// nothing claimed is lowered into navigation the whole frame sees.
pub(crate) fn structural_keys(scale: f64) -> Handler<Editor, DispatchContext<Editor>> {
    let mut handler = Handler::<Editor, DispatchContext<Editor>>::new();
    handler.on_key_with(move |editor, event, input| {
        let geometry = input.geometry(scale);
        editor.paste_key(event)
            || editor.delete_key(geometry, event)
            || editor.insert_key(geometry, event)
            || editor.replace_key(geometry, event)
            || navigate::tab(editor, geometry, event)
            || navigate::select_all_key(editor, geometry, event)
            || navigate::direction(event)
                .is_some_and(|direction| navigate_frame(editor, direction, input, scale))
    });
    handler
}

fn navigate_frame(
    editor: &mut Editor,
    direction: NavigationDirection,
    input: &mut DispatchContext<Editor>,
    scale: f64,
) -> bool {
    let handled = input
        .handler
        .clone()
        .dispatch(editor, Event::Navigate(direction), input)
        .handled();
    if handled {
        input.geometry(scale).reveal_selection(&mut editor.model);
    }
    handled
}

/// Merge `next` into the pending batch, or return the batch it cannot join.
fn queue<T>(
    slot: &mut Option<T>,
    next: T,
    merge: impl FnOnce(&mut T, T) -> Result<(), T>,
) -> Option<T> {
    match slot.take() {
        None => {
            *slot = Some(next);
            None
        }
        Some(mut pending) => match merge(&mut pending, next) {
            Ok(()) => {
                *slot = Some(pending);
                None
            }
            Err(next) => {
                *slot = Some(next);
                Some(pending)
            }
        },
    }
}

impl EditorRunner {
    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn stack_changed(&mut self, stack: crate::stack::Stack, scale: f64, viewport: Size) {
        self.flush_pending_continuous();
        self.editor.replace_stack(stack);
        self.refresh_frame(scale, viewport);
    }

    #[cfg(any(target_os = "macos", target_arch = "wasm32", test))]
    pub(crate) fn palette_changed(
        &mut self,
        palette: crate::display::widget::style::Palette,
        scale: f64,
        viewport: Size,
    ) -> bool {
        if self.editor.palette == palette {
            false
        } else {
            self.flush_pending_continuous();
            self.editor.palette = palette;
            self.refresh_frame(scale, viewport);
            true
        }
    }

    pub(crate) fn focus_changed(&mut self, focused: bool, scale: f64, viewport: Size) -> bool {
        // Alt+Tab away and back is no tap of Alt.
        self.editor.menu.interrupt();
        if self.editor.focused == focused {
            false
        } else {
            self.flush_pending_continuous();
            if !focused {
                self.editor.model.selection = None;
            }
            self.editor.focused = focused;
            if focused {
                self.refresh_frame(scale, viewport);
            } else {
                // Focus loss can take the release away from this window, so
                // the press is forgotten and cancelled. Departure alone does
                // not cancel: Winit forwards macOS drags outside the client
                // area.
                self.editor.reducer = WindowEventReducer::default();
                self.pointer_event(
                    &PointerEvent::Cancel(PointerInfo {
                        pointer_id: Some(PointerId::PRIMARY),
                        persistent_device_id: None,
                        pointer_type: PointerType::Mouse,
                    }),
                    scale,
                    viewport,
                );
            }
            true
        }
    }

    pub(crate) fn modifiers_changed(&mut self, modifiers: Modifiers, scale: f64, viewport: Size) {
        self.update_frame(scale, viewport, |editor, dispatch, hover| {
            editor.modifiers = modifiers;
            dispatch.send(editor, hover, Event::ModifiersChanged(&modifiers));
            true
        });
    }

    pub(crate) fn window_moved(&mut self, scale: f64, viewport: Size) -> bool {
        // Window managers that move windows by Alt+drag take the drag, but
        // its Alt still arrives.
        self.editor.menu.interrupt();
        let changed = self.editor.pointer.take().is_some() || self.frame.hover.is_some();
        if changed {
            self.refresh_frame(scale, viewport);
        }
        changed
    }

    pub(crate) fn keyboard_event(
        &mut self,
        event: &KeyboardEvent,
        scale: f64,
        viewport: Size,
    ) -> bool {
        self.editor.focused
            && self.update_frame(scale, viewport, |editor, dispatch, hover| {
                dispatch.send(editor, hover, Event::Key(event))
            })
    }

    /// A key as a page forwards it, with the modifiers it holds: pending
    /// motion and changed modifiers settle first, and a key the input
    /// method is composing never executes a shortcut or inserts its
    /// preedit. Whether the editor took the key, and whether anything else
    /// changed.
    #[cfg(any(test, feature = "control", target_arch = "wasm32"))]
    pub(crate) fn forwarded_key(
        &mut self,
        event: &KeyboardEvent,
        scale: f64,
        viewport: Size,
    ) -> (bool, bool) {
        let flushed = self.flush_pending_continuous();
        let modifiers_changed = self.editor.modifiers != event.modifiers;
        if modifiers_changed {
            self.modifiers_changed(event.modifiers, scale, viewport);
        }
        let handled = !event.is_composing && self.keyboard_event(event, scale, viewport);
        (handled, flushed || modifiers_changed)
    }

    pub(crate) fn ime_event(&mut self, event: &ImeEvent, scale: f64, viewport: Size) -> bool {
        self.editor.focused
            && self.update_frame(scale, viewport, |editor, dispatch, hover| {
                dispatch.send(editor, hover, Event::Ime(event))
            })
    }

    pub(crate) fn pointer_event(
        &mut self,
        event: &PointerEvent,
        scale: f64,
        viewport: Size,
    ) -> bool {
        let previous_cursor = self.editor.cursor;
        if let Some(position) = pointer_position(event) {
            self.editor.cursor = position;
        }
        match event {
            PointerEvent::Scroll(event) => {
                if let Some(pending) = self.pending_gesture.take() {
                    self.dispatch_batch(pending, |events| Event::Gesture(events));
                }
                if let Some(pending) = queue(
                    &mut self.pending_scroll,
                    PendingScroll {
                        events: vec![event.clone()],
                        scale,
                        viewport,
                    },
                    PendingScroll::merge,
                ) {
                    self.dispatch_batch(pending, |events| Event::Scroll(events));
                }
                true
            }
            PointerEvent::Gesture(event) => {
                // Different continuous operations retain their arrival order.
                // Ordinary pointer refreshes can still coalesce alongside them.
                if let Some(pending) = self.pending_scroll.take() {
                    self.dispatch_batch(pending, |events| Event::Scroll(events));
                }
                if let Some(pending) = queue(
                    &mut self.pending_gesture,
                    PendingGesture {
                        events: vec![event.clone()],
                        scale,
                        viewport,
                    },
                    PendingGesture::merge,
                ) {
                    self.dispatch_batch(pending, |events| Event::Gesture(events));
                }
                true
            }
            PointerEvent::Move(event) => {
                if let Some(pending) = queue(
                    &mut self.pending_pointer,
                    PendingPointer {
                        event: event.clone(),
                        start: previous_cursor,
                        scale,
                        viewport,
                    },
                    PendingPointer::merge,
                ) {
                    self.dispatch_pointer_batch(&pending);
                }
                self.editor.cursor = Point::new(event.current.position.x, event.current.position.y);
                self.editor.pointer = window_pointer(self.editor.cursor, viewport);
                self.editor.modifiers = event.current.modifiers;
                true
            }
            _ => {
                // Touch has no preceding hover motion. Establish its target
                // before dispatching the press through the usual handlers.
                if let PointerEvent::Down(button) = event
                    && button.pointer.pointer_type == PointerType::Touch
                {
                    self.editor.pointer = pointer_position(event);
                    self.refresh_frame(scale, viewport);
                }
                // Presses, releases and the pointer leaving change the frame's
                // input, so each mints a successor whatever handles it.
                self.update_frame(scale, viewport, |editor, dispatch, hover| match event {
                    PointerEvent::Down(button) => {
                        editor.pointer = window_pointer(editor.cursor, viewport);
                        editor.pressed = true;
                        editor.finish_gesture();
                        // A primary press nothing takes, over nothing, deselects.
                        if !dispatch.send(editor, hover, Event::PointerDown(button))
                            && puri::interact::is_primary_contact(button)
                            && hover.is_none()
                        {
                            editor.model.selection = None;
                        }
                        true
                    }
                    PointerEvent::Up(button) => {
                        editor.pointer = window_pointer(editor.cursor, viewport);
                        editor.pressed = false;
                        // A running gesture takes its release.
                        if !editor.finish_gesture() {
                            dispatch.send(editor, hover, Event::PointerUp(button));
                        }
                        true
                    }
                    PointerEvent::Leave(_) => {
                        editor.pointer = None;
                        true
                    }
                    PointerEvent::Cancel(pointer) => {
                        editor.pointer = None;
                        editor.pressed = false;
                        dispatch.send(editor, hover, Event::PointerCancel(pointer));
                        editor.model.workspace.cancel_resize();
                        editor.finish_gesture();
                        true
                    }
                    _ => false,
                })
            }
        }
    }

    /// Dispatch a held batch as the one `event` it makes, through the frame
    /// it was held for.
    fn dispatch_batch<T: Clone>(
        &mut self,
        pending: PendingBatch<T>,
        event: impl for<'a> FnOnce(Cow<'a, [T]>) -> Event<'a>,
    ) -> bool {
        self.update_frame(
            pending.scale,
            pending.viewport,
            |editor, dispatch, hover| {
                dispatch.send(editor, hover, event(Cow::Borrowed(&pending.events)))
            },
        )
    }

    fn dispatch_pointer_batch(&mut self, pending: &PendingPointer) -> bool {
        self.editor.cursor = Point::new(
            pending.event.current.position.x,
            pending.event.current.position.y,
        );
        self.editor.pointer = window_pointer(self.editor.cursor, pending.viewport);
        self.editor.modifiers = pending.event.current.modifiers;
        self.probe_pointer(pending.scale);
        let hover_handled = self.dispatch_hover_changed();
        self.update_frame(
            pending.scale,
            pending.viewport,
            |editor, dispatch, hover| {
                let event = &pending.event;
                let samples: Vec<_> = puri::interact::pointer_samples(event)
                    .map(|sample| Point::new(sample.position.x, sample.position.y))
                    .collect();
                let moved = editor.advance_gesture(&samples)
                    || dispatch.send(editor, hover, Event::PointerMove(event));
                // Unclaimed touch motion scrolls through the same nested handlers.
                let handled = moved
                    || (event.pointer.pointer_type == PointerType::Touch && {
                        let scroll = puri::interact::pointer_samples(event)
                            .scan(pending.start, |previous, sample| {
                                let point = Point::new(sample.position.x, sample.position.y);
                                let delta = point - *previous;
                                *previous = point;
                                Some(PointerScrollEvent {
                                    pointer: event.pointer,
                                    delta: ScrollDelta::PixelDelta((delta.x, delta.y).into()),
                                    state: sample.clone(),
                                })
                            })
                            .collect();
                        dispatch.send(editor, hover, Event::Scroll(Cow::Owned(scroll)))
                    });
                handled || hover_handled
            },
        )
    }

    /// Scroll/gestures settle geometry before motion is dispatched. The successor
    /// already includes the latest pointer input; unhandled paired motion
    /// therefore needs no second frame. Without such a frame, motion must
    /// still refresh hover even if no handler accepts it.
    pub(crate) fn flush_pending_continuous(&mut self) -> bool {
        let pointer = self.pending_pointer.take();
        let mut reminted = match self.pending_scroll.take() {
            Some(pending) => self.dispatch_batch(pending, |events| Event::Scroll(events)),
            None => false,
        };
        if let Some(pending) = self.pending_gesture.take() {
            reminted |= self.dispatch_batch(pending, |events| Event::Gesture(events));
        }
        if let Some(pointer) = pointer {
            reminted |= self.dispatch_pointer_batch(&pointer);
            if !reminted {
                self.refresh_frame(pointer.scale, pointer.viewport);
                reminted = true;
            }
        }
        reminted
    }
}

#[cfg(test)]
mod tests;
