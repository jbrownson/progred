use crate::display::host::{Host, Layout};
use crate::display::widget;
use crate::display::widget::frame::Cursor;
use crate::display::widget::gesture::Gesture;
use crate::gesture::EditRun;
use gid::Value;
use kurbo::Point;
use puri::drag::Drag;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScrubEvent {
    pub movement_x: f64,
    pub distance_y: f64,
}

pub(crate) struct ScrubUpdate {
    pub value: Value,
    pub spelling: String,
}

pub(crate) type ScrubGesture = Box<dyn FnMut(ScrubEvent) -> ScrubUpdate>;
pub(crate) type ScrubHandler = Rc<dyn Fn() -> ScrubGesture>;

pub(crate) fn on_scrub<W: Host>(
    child: Layout<W>,
    target: W::Hover,
    handler: ScrubHandler,
) -> Layout<W> {
    widget::before(
        child,
        Rc::new(move |context| {
            if context.viewing().writable() {
                let site = context.site();
                let handler = handler.clone();
                let scale = context.viewing().scale();
                let target = target.clone();
                let drag =
                    widget::gesture::targeted(target.clone(), true, move |world: &mut W, point| {
                        let edit = site.edit_run();
                        let mut session = world.session();
                        if edit.select(&mut session) {
                            session.start_gesture(
                                scrub(
                                    Drag::new(point, scale, crate::gesture::DRAG_THRESHOLD),
                                    handler(),
                                    edit,
                                ),
                                &[],
                            );
                            true
                        } else {
                            false
                        }
                    });
                Box::new(move |output, placement| {
                    if output.input.picking && !placement.clipped_out() {
                        output.cursor(target, Cursor::HorizontalDrag);
                    }
                    drag(output, placement);
                })
            } else {
                Box::new(|_, _| {})
            }
        }),
    )
}

struct Scrub {
    drag: Drag,
    update: ScrubGesture,
    edit: EditRun,
    edited_spelling: bool,
}

pub(crate) fn scrub(drag: Drag, update: ScrubGesture, edit: EditRun) -> Box<dyn Gesture> {
    Box::new(Scrub {
        drag,
        update,
        edit,
        edited_spelling: false,
    })
}

impl Gesture for Scrub {
    fn advance(&mut self, session: &mut crate::editing::Session<'_>, samples: &[Point]) -> bool {
        samples.iter().fold(false, |changed, point| {
            self.drag.advance(*point).is_some_and(|motion| {
                let update = (self.update)(ScrubEvent {
                    movement_x: motion.movement.x,
                    distance_y: motion.distance.y,
                });
                let wrote = self.edit.write(session, update.value);
                if let Some(selected) = self.edit.selected(session) {
                    *selected.edit_line_mut(&update.spelling) =
                        puri::edit::LineEditState::new(&update.spelling).with_cursor_at_end();
                    self.edited_spelling = true;
                }
                wrote
            }) || changed
        })
    }

    fn cursor(&self) -> Option<Cursor> {
        Some(Cursor::HorizontalDrag)
    }

    fn finish(&mut self, session: &mut crate::editing::Session<'_>) {
        if self.edited_spelling
            && let Some(selected) = self.edit.selected(session)
        {
            selected.clear_editor();
        }
    }
}

#[cfg(test)]
#[path = "tests/scrub.rs"]
mod tests;
