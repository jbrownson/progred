use super::{Annotate, HoverCallback, before};
use crate::display::host::Layout;
use crate::display::{Action, Host};
use gid::Value;
use puri::drag::Drag;
use puri::handler::HasHandler;
use puri::interact::is_primary_contact;
use puri::{Point, Rect};
use std::rc::Rc;

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

pub trait Gesture {
    /// Document writes return true; annotation-only changes do not.
    /// This does not control event propagation: an active gesture owns motion.
    fn advance(&mut self, session: &mut crate::editing::Session<'_>, samples: &[Point]) -> bool;

    fn finish(&mut self, _session: &mut crate::editing::Session<'_>) {}

    /// The pointer's look while this runs, wherever it goes.
    fn cursor(&self) -> Option<super::frame::Cursor> {
        None
    }
}

pub fn targeted<World: 'static, Hover: 'static>(
    target: Hover,
    pick: bool,
    same_target: fn(&Hover, &Hover) -> bool,
    start: impl Fn(&mut World, Point) -> bool + 'static,
) -> HoverCallback<World, Hover> {
    Box::new(move |output, placement| {
        let picking = crate::modifiers::picking(output.input.command_modifier);
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

pub fn on_state_drag<W: Host>(
    child: Layout<W>,
    target: W::Hover,
    on_press: Action,
    handler: StateDragHandler,
) -> Layout<W> {
    before(
        child,
        Rc::new(move |context| {
            let site = context.site();
            let annotate: Annotate =
                Rc::new(move |session, value| site.open(session).set_state(value));
            let handler = handler.clone();
            let on_press = on_press.clone();
            let scale = context.toolkit.styles().scale;
            let threshold = crate::gesture::DRAG_THRESHOLD;
            targeted(
                target.clone(),
                false,
                PartialEq::eq,
                move |world: &mut W, point| {
                    let mut session = world.session();
                    if on_press(&mut session) {
                        session.start_gesture(
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

/// While the command modifier is held, what can be dragged here is outlined
/// as hovering outlines a value, in the drag color: every place the pointer
/// can drag shows at once.
pub fn drag_outlined<W: 'static, H: 'static>(
    child: crate::display::Layout<W, H>,
) -> crate::display::Layout<W, H> {
    super::around(
        child,
        Rc::new(|context| {
            let writable = context.viewing().writable();
            let scale = context.toolkit.styles().scale;
            let palette = context.toolkit.styles().palette;
            Box::new(move |child| {
                if !writable {
                    return child;
                }
                super::before_place(child, move |placement, output| {
                    if output.input.picking && !placement.clipped_out() {
                        let outline = super::style::highlight_outline(scale, placement.rect);
                        output.render(move |canvas, _| {
                            super::style::drag_highlight(palette, scale, canvas, outline);
                        });
                    }
                })
            })
        }),
    )
}

pub fn on_point<W: Host>(child: Layout<W>, handler: PointHandler) -> Layout<W> {
    before(
        child,
        Rc::new(move |context| {
            if context.viewing().writable() {
                let site = context.site();
                let handler = handler.clone();
                Box::new(move |output, placement| {
                    output.handler().on_pointer_down(move |world, event| {
                        let at = Point::new(event.state.position.x, event.state.position.y);
                        if is_primary_contact(event) && placement.contains(at) {
                            world.session().start_gesture(
                                point(placement.rect, handler.clone(), site.edit_run()),
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

struct StateDrag {
    drag: Drag,
    update: crate::display::StateDragGesture,
    annotate: Annotate,
}

pub fn state_drag(
    drag: Drag,
    update: crate::display::StateDragGesture,
    annotate: Annotate,
) -> Box<dyn Gesture> {
    Box::new(StateDrag {
        drag,
        update,
        annotate,
    })
}

impl Gesture for StateDrag {
    fn advance(&mut self, session: &mut crate::editing::Session<'_>, samples: &[Point]) -> bool {
        let samples: Vec<_> = samples
            .iter()
            .filter_map(|point| self.drag.advance(*point))
            .map(|motion| crate::display::StateDragEvent {
                delta_x: motion.distance.x,
                delta_y: motion.distance.y,
            })
            .collect();
        if let Some((current, coalesced)) = samples.split_last() {
            (self.annotate)(session, (self.update)(*current, coalesced));
        }
        false
    }
}

struct PointControl {
    rect: Rect,
    update: PointHandler,
    edit: crate::gesture::EditRun,
}

pub(crate) fn point(
    rect: Rect,
    update: PointHandler,
    edit: crate::gesture::EditRun,
) -> Box<dyn Gesture> {
    Box::new(PointControl { rect, update, edit })
}

impl Gesture for PointControl {
    fn advance(&mut self, session: &mut crate::editing::Session<'_>, samples: &[Point]) -> bool {
        samples.iter().fold(false, |changed, point| {
            let update = (self.update)(crate::display::PointEvent {
                x: ((point.x - self.rect.x0) / self.rect.width()).clamp(0.0, 1.0),
                y: ((point.y - self.rect.y0) / self.rect.height()).clamp(0.0, 1.0),
            });
            let wrote = self.edit.write(session, update.value);
            if let Some(payload) = update.selection {
                self.edit.selection(session, payload);
            }
            wrote || changed
        })
    }
}

#[cfg(test)]
mod tests;
