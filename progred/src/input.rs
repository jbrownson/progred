use crate::frame::{DispatchContext, frame_disposition};
#[cfg(test)]
use crate::selection;
use crate::{Editor, EditorRunner, navigate};
use kurbo::{Point, Rect, Size};
use puri::handler::{Event, Handler, ImeEvent, NavigationDirection};
use std::borrow::Cow;
use ui_events::ScrollDelta;
use ui_events::keyboard::{KeyboardEvent, Modifiers};
use ui_events::pointer::{
    PointerEvent, PointerGestureEvent, PointerScrollEvent, PointerType, PointerUpdate,
};
use winit::event::WindowEvent;

pub(crate) struct PendingPaint {
    pub(crate) scale: f64,
    pub(crate) viewport: Size,
    pub(crate) renders: Vec<puri::frame::Render>,
}

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

pub(super) fn pointer_position(event: &PointerEvent) -> Option<Point> {
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

pub(super) fn window_pointer(position: Point, size: Size) -> Option<Point> {
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
            || navigate::keyboard(editor, geometry, event)
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
    pub(crate) fn stack_changed(
        &mut self,
        stack: crate::stack::Stack<crate::Editor>,
        scale: f64,
        viewport: Size,
    ) {
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
            } else if let Some(ui_events_winit::WindowEventTranslation::Pointer(event)) =
                crate::translate_window_event(
                    &mut self.editor.reducer,
                    scale,
                    &winit::event::WindowEvent::Focused(false),
                )
            {
                self.pointer_event(&event, scale, viewport);
            }
            true
        }
    }

    pub(crate) fn modifiers_changed(&mut self, modifiers: Modifiers, scale: f64, viewport: Size) {
        self.update_frame(scale, viewport, |editor, dispatch, hover| {
            editor.modifiers = modifiers;
            dispatch.handler.dispatch(
                editor,
                Event::ModifiersChanged(&modifiers),
                &mut dispatch.context(editor, hover.cloned()),
            );
            frame_disposition(false, true)
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
                frame_disposition(
                    dispatch.handler.dispatch_key_with(
                        editor,
                        event,
                        &mut dispatch.context(editor, hover.cloned()),
                    ),
                    false,
                )
            })
    }

    pub(crate) fn ime_event(&mut self, event: &ImeEvent, scale: f64, viewport: Size) -> bool {
        self.editor.focused
            && self.update_frame(scale, viewport, |editor, dispatch, hover| {
                frame_disposition(
                    dispatch
                        .handler
                        .dispatch(
                            editor,
                            Event::Ime(event),
                            &mut dispatch.context(editor, hover.cloned()),
                        )
                        .handled(),
                    false,
                )
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
                    self.dispatch_gesture_batch(pending);
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
                    self.dispatch_scroll_batch(pending);
                }
                true
            }
            PointerEvent::Gesture(event) => {
                // Different continuous operations retain their arrival order.
                // Ordinary pointer refreshes can still coalesce alongside them.
                if let Some(pending) = self.pending_scroll.take() {
                    self.dispatch_scroll_batch(pending);
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
                    self.dispatch_gesture_batch(pending);
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
                self.update_frame(scale, viewport, |editor, dispatch, hover| {
                    let (handled, input_changed) = match event {
                        PointerEvent::Down(button) => {
                            editor.pointer = window_pointer(editor.cursor, viewport);
                            editor.pressed = true;
                            editor.finish_gesture();
                            let mut pointer = dispatch.context(editor, hover.cloned());
                            let handled = dispatch.handler.dispatch_pointer_down_with(
                                editor,
                                button,
                                &mut pointer,
                            );
                            (
                                handled
                                    || (puri::interact::is_primary_contact(button)
                                        && pointer.hovered.is_none()
                                        && editor.model.selection.take().is_some()),
                                true,
                            )
                        }
                        PointerEvent::Up(button) => {
                            editor.pointer = window_pointer(editor.cursor, viewport);
                            editor.pressed = false;
                            (
                                editor.finish_gesture()
                                    || dispatch
                                        .handler
                                        .dispatch(
                                            editor,
                                            Event::PointerUp(button),
                                            &mut dispatch.context(editor, hover.cloned()),
                                        )
                                        .handled(),
                                true,
                            )
                        }
                        PointerEvent::Leave(_) => {
                            editor.pointer = None;
                            (false, true)
                        }
                        PointerEvent::Cancel(pointer) => {
                            editor.pointer = None;
                            editor.pressed = false;
                            let handled = dispatch
                                .handler
                                .dispatch(
                                    editor,
                                    Event::PointerCancel(pointer),
                                    &mut dispatch.context(editor, hover.cloned()),
                                )
                                .handled();
                            let resize_cancelled = editor.model.workspace.cancel_resize();
                            let gesture_cancelled = editor.finish_gesture();
                            (handled || resize_cancelled || gesture_cancelled, true)
                        }
                        _ => (false, false),
                    };
                    frame_disposition(handled, input_changed)
                })
            }
        }
    }

    fn dispatch_scroll_batch(&mut self, pending: PendingScroll) -> bool {
        self.update_frame(
            pending.scale,
            pending.viewport,
            |editor, dispatch, hover| {
                let mut input = dispatch.context(editor, hover.cloned());
                frame_disposition(
                    dispatch
                        .handler
                        .dispatch(
                            editor,
                            Event::Scroll(Cow::Borrowed(&pending.events)),
                            &mut input,
                        )
                        .handled(),
                    false,
                )
            },
        )
    }

    fn dispatch_gesture_batch(&mut self, pending: PendingGesture) -> bool {
        self.update_frame(
            pending.scale,
            pending.viewport,
            |editor, dispatch, hover| {
                frame_disposition(
                    dispatch
                        .handler
                        .dispatch(
                            editor,
                            Event::Gesture(Cow::Borrowed(&pending.events)),
                            &mut dispatch.context(editor, hover.cloned()),
                        )
                        .handled(),
                    false,
                )
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
                let mut input = dispatch.context(editor, hover.cloned());
                let samples: Vec<_> = puri::interact::pointer_samples(event)
                    .map(|sample| Point::new(sample.position.x, sample.position.y))
                    .collect();
                let moved = editor.advance_gesture(&samples)
                    || dispatch
                        .handler
                        .dispatch(editor, Event::PointerMove(event), &mut input)
                        .handled();
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
                        dispatch
                            .handler
                            .dispatch(editor, Event::Scroll(Cow::Owned(scroll)), &mut input)
                            .handled()
                    });
                frame_disposition(handled || hover_handled, false)
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
            Some(pending) => self.dispatch_scroll_batch(pending),
            None => false,
        };
        if let Some(pending) = self.pending_gesture.take() {
            reminted |= self.dispatch_gesture_batch(pending);
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
