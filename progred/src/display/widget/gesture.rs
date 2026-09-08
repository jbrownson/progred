use super::{Annotate, HoverCallback, before};
use crate::display::{ActionHandler, Layout};
use gid::Value;
use puri::drag::Drag;
use puri::handler::{HasHandler, PointerButtonEvent};
use puri::interact::is_primary_contact;
use puri::{Point, Rect};
use std::rc::Rc;

/// Logical drag motion after the shared threshold recognizer accepts it.
/// Number libraries map this motion to values without inspecting raw buttons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrubEvent {
    pub movement_x: f64,
    pub distance_y: f64,
}

pub struct ScrubUpdate {
    pub value: Value,
    pub spelling: Option<String>,
}

pub type ScrubGesture = Box<dyn FnMut(ScrubEvent) -> ScrubUpdate>;
pub type ScrubHandler = Rc<dyn Fn() -> ScrubGesture>;

/// Displacement from the drag's origin, in logical display units.
/// The caller supplies threshold policy and retains the active continuation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StateDragEvent {
    pub delta_x: f64,
    pub delta_y: f64,
}

/// The latest displacement and earlier samples in order. A handler may
/// use just the latest displacement or integrate the complete path.
pub type StateDragGesture = Box<dyn FnMut(StateDragEvent, &[StateDragEvent]) -> Value>;
pub type StateDragHandler = Rc<dyn Fn() -> StateDragGesture>;

/// A position inside a continuous two-dimensional control, normalized
/// to its settled rectangle. The host owns pointer capture and writes
/// the returned value through the projected location.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointEvent {
    pub x: f64,
    pub y: f64,
}

pub struct PointUpdate {
    pub value: Value,
    /// Optional replacement for the selected location's transient
    /// payload. This is control state, not document data.
    pub selection: Option<Value>,
}

pub type PointHandler = Rc<dyn Fn(PointEvent) -> PointUpdate>;

pub trait Gesture<World> {
    /// Document writes return true; annotation-only changes do not.
    /// This does not control event propagation: an active gesture owns motion.
    fn advance(&mut self, world: &mut World, samples: &[Point]) -> bool;

    fn spelling(&self) -> Option<&str> {
        None
    }
}

pub type Write<World> = Box<dyn FnMut(&mut World, Value) -> bool>;

/// A fresh edit run. The writer owns grouping; the payload setter only updates
/// an existing selection at this site. Neither operation implicitly selects it.
pub struct ValueEdit<World> {
    pub select: ActionHandler<World>,
    pub write: Write<World>,
    pub selection: Rc<dyn Fn(&mut World, Value)>,
}

pub fn targeted<World: 'static, Hover: 'static>(
    target: Hover,
    pick: bool,
    picking: fn(&PointerButtonEvent) -> bool,
    same_target: fn(&Hover, &Hover) -> bool,
    start: impl Fn(&mut World, Point) -> bool + 'static,
) -> HoverCallback<World, Hover> {
    Box::new(move |output, placement| {
        output
            .handler()
            .on_pointer_down_with(move |world, event, hovered| {
                let point = Point::new(event.state.position.x, event.state.position.y);
                is_primary_contact(event)
                    && picking(event) == pick
                    && placement.contains(point)
                    && hovered
                        .hovered()
                        .is_some_and(|hover| same_target(hover, &target))
                    && start(world, point)
            });
    })
}

pub fn on_scrub(
    child: Layout<crate::Editor, crate::frame::Hovered>,
    target: crate::frame::Hovered,
    handler: ScrubHandler,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    before(
        child,
        Rc::new(move |context| {
            if !context.inputs.source.transient()
                && crate::selection::writable_at(&context.inputs.sources, context.path)
            {
                let root = context.inputs.view.clone();
                let path = context.path.to_vec();
                let handler = handler.clone();
                let scale = context.inputs.styles.scale;
                let threshold = crate::gesture::DRAG_THRESHOLD;
                targeted(
                    target.clone(),
                    true,
                    crate::editing::picking,
                    PartialEq::eq,
                    move |world, point| {
                        let edit = crate::gesture::value_edit(root.clone(), path.clone());
                        if (edit.select)(world) {
                            crate::editing::start_gesture(
                                world,
                                root.clone(),
                                path.clone(),
                                scrub(Drag::new(point, scale, threshold), handler(), edit.write),
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

pub fn on_state_drag(
    child: Layout<crate::Editor, crate::frame::Hovered>,
    target: crate::frame::Hovered,
    on_press: ActionHandler<crate::Editor>,
    handler: StateDragHandler,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    before(
        child,
        Rc::new(move |context| {
            let root = context.inputs.view.clone();
            let path = context.path.to_vec();
            let annotation_root = root.clone();
            let annotation_path = path.clone();
            let annotate: Annotate<crate::Editor> = Rc::new(move |world, value| {
                crate::editing::annotate(world, &annotation_root, &annotation_path, value)
            });
            let handler = handler.clone();
            let on_press = on_press.clone();
            let scale = context.inputs.styles.scale;
            let threshold = crate::gesture::DRAG_THRESHOLD;
            targeted(
                target.clone(),
                false,
                crate::editing::picking,
                PartialEq::eq,
                move |world, point| {
                    if on_press(world) {
                        crate::editing::start_gesture(
                            world,
                            root.clone(),
                            path.clone(),
                            state_drag(
                                Drag::new(point, scale, threshold),
                                handler(),
                                annotate.clone(),
                            ),
                            &[],
                        );
                        true
                    } else {
                        false
                    }
                },
            )
        }),
    )
}

pub fn on_point(
    child: Layout<crate::Editor, crate::frame::Hovered>,
    handler: PointHandler,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    before(
        child,
        Rc::new(move |context| {
            if !context.inputs.source.transient()
                && crate::selection::writable_at(&context.inputs.sources, context.path)
            {
                let root = context.inputs.view.clone();
                let path = context.path.to_vec();
                let handler = handler.clone();
                Box::new(move |output, placement| {
                    output.handler().on_pointer_down(move |world, event| {
                        let at = Point::new(event.state.position.x, event.state.position.y);
                        if is_primary_contact(event) && placement.contains(at) {
                            crate::editing::start_gesture(
                                world,
                                root.clone(),
                                path.clone(),
                                point(
                                    placement.rect,
                                    handler.clone(),
                                    crate::gesture::value_edit(root.clone(), path.clone()),
                                ),
                                &[at],
                            );
                            true
                        } else {
                            false
                        }
                    });
                })
            } else {
                Box::new(|_, _| {})
            }
        }),
    )
}

struct Scrub<World> {
    drag: Drag,
    update: crate::display::ScrubGesture,
    write: Write<World>,
    spelling: Option<String>,
}

pub fn scrub<World: 'static>(
    drag: Drag,
    update: crate::display::ScrubGesture,
    write: Write<World>,
) -> Box<dyn Gesture<World>> {
    Box::new(Scrub {
        drag,
        update,
        write,
        spelling: None,
    })
}

impl<World> Gesture<World> for Scrub<World> {
    fn advance(&mut self, world: &mut World, samples: &[Point]) -> bool {
        samples.iter().fold(false, |changed, point| {
            self.drag.advance(*point).is_some_and(|motion| {
                let update = (self.update)(crate::display::ScrubEvent {
                    movement_x: motion.movement.x,
                    distance_y: motion.distance.y,
                });
                self.spelling = update.spelling;
                (self.write)(world, update.value)
            }) || changed
        })
    }

    fn spelling(&self) -> Option<&str> {
        self.spelling.as_deref()
    }
}

struct StateDrag<World> {
    drag: Drag,
    update: crate::display::StateDragGesture,
    annotate: Annotate<World>,
}

pub fn state_drag<World: 'static>(
    drag: Drag,
    update: crate::display::StateDragGesture,
    annotate: Annotate<World>,
) -> Box<dyn Gesture<World>> {
    Box::new(StateDrag {
        drag,
        update,
        annotate,
    })
}

impl<World> Gesture<World> for StateDrag<World> {
    fn advance(&mut self, world: &mut World, samples: &[Point]) -> bool {
        let samples: Vec<_> = samples
            .iter()
            .filter_map(|point| self.drag.advance(*point))
            .map(|motion| crate::display::StateDragEvent {
                delta_x: motion.distance.x,
                delta_y: motion.distance.y,
            })
            .collect();
        if let Some((current, coalesced)) = samples.split_last() {
            (self.annotate)(world, (self.update)(*current, coalesced));
        }
        false
    }
}

struct PointControl<World> {
    rect: Rect,
    update: PointHandler,
    edit: ValueEdit<World>,
}

pub fn point<World: 'static>(
    rect: Rect,
    update: PointHandler,
    edit: ValueEdit<World>,
) -> Box<dyn Gesture<World>> {
    Box::new(PointControl { rect, update, edit })
}

impl<World> Gesture<World> for PointControl<World> {
    fn advance(&mut self, world: &mut World, samples: &[Point]) -> bool {
        samples.iter().fold(false, |changed, point| {
            let update = (self.update)(crate::display::PointEvent {
                x: ((point.x - self.rect.x0) / self.rect.width()).clamp(0.0, 1.0),
                y: ((point.y - self.rect.y0) / self.rect.height()).clamp(0.0, 1.0),
            });
            let wrote = (self.edit.write)(world, update.value);
            if let Some(payload) = update.selection {
                (self.edit.selection)(world, payload);
            }
            wrote || changed
        })
    }
}
