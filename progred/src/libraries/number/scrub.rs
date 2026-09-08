use crate::display::widget::gesture::Gesture;
use crate::display::{Layout, widget};
use crate::gesture::EditRun;
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
    child: Layout<crate::Editor, crate::frame::Hovered>,
    target: crate::frame::Hovered,
    handler: ScrubHandler,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    widget::before(
        child,
        Rc::new(move |context| {
            if !context.inputs.source.transient()
                && crate::selection::writable_at(&context.inputs.sources, context.path)
            {
                let root = context.inputs.view.clone();
                let path = context.path.to_vec();
                let handler = handler.clone();
                let scale = context.inputs.styles.scale;
                widget::gesture::targeted(
                    target.clone(),
                    true,
                    crate::editing::picking,
                    PartialEq::eq,
                    move |world, point| {
                        let edit = crate::gesture::value_edit(root.clone(), path.clone());
                        if edit.select(world) {
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
                )
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

pub(crate) fn scrub(
    drag: Drag,
    update: ScrubGesture,
    edit: EditRun,
) -> Box<dyn Gesture<crate::Editor>> {
    Box::new(Scrub {
        drag,
        update,
        edit,
        edited_spelling: false,
    })
}

impl Gesture<crate::Editor> for Scrub {
    fn advance(&mut self, world: &mut crate::Editor, samples: &[Point]) -> bool {
        samples.iter().fold(false, |changed, point| {
            self.drag.advance(*point).is_some_and(|motion| {
                let update = (self.update)(ScrubEvent {
                    movement_x: motion.movement.x,
                    distance_y: motion.distance.y,
                });
                let wrote = self.edit.write(world, update.value);
                if let Some(spelling) = update.spelling
                    && let Some(selected) = self.edit.selected(world)
                {
                    *selected.edit_line_mut(&spelling) =
                        puri::edit::LineEditState::new(&spelling).with_cursor_at_end();
                    self.edited_spelling = true;
                }
                wrote
            }) || changed
        })
    }

    fn finish(&mut self, world: &mut crate::Editor) {
        if self.edited_spelling
            && let Some(selected) = self.edit.selected(world)
        {
            selected.clear_editor();
        }
    }
}
