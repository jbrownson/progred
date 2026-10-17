use crate::display::widget;
use crate::display::widget::gesture::Gesture;
use crate::gesture::EditRun;
use crate::placed::Cursor;
use crate::view::Layout;
use gid::Value;
use puri::{Point, drag::Drag};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScrubEvent {
    pub movement_x: f64,
    pub distance_y: f64,
}

pub(crate) struct ScrubUpdate {
    pub value: Value,
    pub spelling: Option<String>,
}

pub(crate) type ScrubGesture = Box<dyn FnMut(ScrubEvent) -> ScrubUpdate>;
pub(crate) type ScrubHandler = Rc<dyn Fn() -> ScrubGesture>;

pub(crate) fn on_scrub(
    child: Layout,
    target: crate::frame::Hovered,
    handler: ScrubHandler,
) -> Layout {
    widget::before(
        child,
        Rc::new(move |context| {
            if context
                .inputs
                .edits
                .writable(&context.inputs.sources, context.path)
            {
                let root = context.inputs.view.clone();
                let path = context.path.to_vec();
                let edits = context.inputs.edits.clone();
                let handler = handler.clone();
                let scale = context.inputs.styles.scale;
                let target = target.clone();
                let drag = widget::gesture::targeted(
                    target.clone(),
                    true,
                    PartialEq::eq,
                    move |world: &mut crate::Editor, point| {
                        let edit = crate::gesture::scoped_value_edit(
                            root.clone(),
                            path.clone(),
                            edits.clone(),
                        );
                        if edit.select(&mut world.session()) {
                            crate::editing::start_gesture(
                                world,
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
                    },
                );
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

/// While the command modifier is held, a number that holding it lets be
/// dragged is outlined as hovering outlines a value, in the number's own
/// color: every place the pointer can drag shows at once.
pub(crate) fn outlined(digits: Layout) -> Layout {
    widget::around(
        digits,
        Rc::new(|context| {
            let writable = context
                .inputs
                .edits
                .writable(&context.inputs.sources, context.path);
            let scale = context.inputs.styles.scale;
            let palette = context.inputs.styles.palette;
            Box::new(move |digits| {
                if !writable {
                    return digits;
                }
                widget::before_place(digits, move |placement, output| {
                    if output.input.picking && !placement.clipped_out() {
                        let outline = widget::style::highlight_outline(scale, placement.rect);
                        output.render(move |canvas, _| {
                            widget::style::drag_highlight(palette, scale, canvas, outline);
                        });
                    }
                })
            })
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
                if let Some(spelling) = update.spelling
                    && let Some(selected) = self.edit.selected(session)
                {
                    *selected.edit_line_mut(&spelling) =
                        puri::edit::LineEditState::new(&spelling).with_cursor_at_end();
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
mod tests;
