//! Containers compose placement outputs without knowing which widgets produced them.

use measured::{Extent, Measured, Output};
use puri::handler::{Event, EventOutcome, Handler, HasHandler, PointerScrollEvent, ScrollOutcome};
use puri::{Placement, Point, Vec2};

pub trait Layers: Output {
    fn clipped(self, placement: Placement) -> Self;
    fn float(&mut self, above: Self);
}

pub fn floating<O: Layers + 'static>(
    base: Measured<O>,
    content: Measured<O>,
    place: impl FnOnce(Placement, Extent) -> Option<Placement> + 'static,
) -> Measured<O> {
    let extent = content.extent;
    measured::around(base, move |placement, base| {
        let mut output = base.place();
        if let Some(placement) = place(placement, extent) {
            output.float(measured::place(content, placement));
        }
        output
    })
}

/// The caller owns the offset. Only gesture starts and scroll are bounded;
/// motion, release, and keyboard events remain available to active controls.
pub fn scrolled<World: 'static, O: Layers + HasHandler<World> + 'static>(
    child: Measured<O>,
    offset: Vec2,
    on_scroll: impl Fn(&mut World, &PointerScrollEvent) -> ScrollOutcome + 'static,
) -> Measured<O> {
    measured::around(child, move |placement, inner| {
        let mut base = O::empty();
        if !placement.clipped_out() {
            base.handler().on_scroll(move |state, event| {
                if placement.contains(Point::new(event.state.position.x, event.state.position.y)) {
                    on_scroll(state, event)
                } else {
                    ScrollOutcome::pass(event)
                }
            });
        }
        let child_rect = inner.extent().rect_at(placement.rect.origin() - offset);
        let child_placement = measured::child_placement(
            measured::clipped_placement(placement, placement.rect),
            child_rect,
        );
        base.over(inner.place_at(child_placement).clipped(placement))
    })
}

pub fn gate_starts<World: 'static, Input: 'static>(
    child: Handler<World, Input>,
    placement: Placement,
) -> Handler<World, Input> {
    Handler::from_function(move |world, event, input| {
        let position = match &event {
            Event::PointerDown(event) => Some(event.state.position),
            Event::Scroll(event) => Some(event.state.position),
            _ => None,
        };
        if position.is_some_and(|point| !placement.contains(Point::new(point.x, point.y))) {
            EventOutcome::decline(event)
        } else {
            child.dispatch(world, event, input)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::{Fragment, leaf};
    use puri::draw::{DrawCmd, DrawList, Shape};
    use puri::handler::{PointerButtonEvent, PointerInfo, PointerState, PointerType, ScrollDelta};
    use puri::{Affine, Color, Rect};
    use std::{cell::RefCell, rc::Rc};

    type Frame = Fragment<Vec<&'static str>, u8>;

    fn extent() -> Extent {
        Extent {
            width: 100.0,
            ascent: 20.0,
            descent: 80.0,
        }
    }

    fn pointer_at(x: f64, y: f64) -> PointerButtonEvent {
        PointerButtonEvent {
            button: None,
            pointer: PointerInfo {
                pointer_id: None,
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state: PointerState {
                position: (x, y).into(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn scroll_shifts_content_and_clips_starts_but_not_active_input() {
        let seen = Rc::new(RefCell::new(None));
        let capture = seen.clone();
        let child = leaf(extent(), move |output: &mut Frame, placement| {
            *capture.borrow_mut() = Some(placement);
            output.handler().on_pointer_down(|log, _| {
                log.push("down");
                true
            });
            output.handler().on_pointer_up(|log, _| {
                log.push("up");
                true
            });
            output.handler().on_key(|log, _| {
                log.push("key");
                true
            });
            output.render(move |canvas, _| {
                canvas.fill_shape(placement.rect.into(), Color::BLACK.into(), Affine::IDENTITY);
            });
        });
        let viewport = Placement::new(
            Rect::new(10.0, 20.0, 50.0, 60.0),
            Rect::new(0.0, 0.0, 30.0, 100.0),
        );
        let frame = measured::place(
            scrolled(child, Vec2::new(5.0, 15.0), |log, event| {
                log.push("scroll");
                ScrollOutcome::consume(event)
            }),
            viewport,
        );
        assert_eq!(
            *seen.borrow(),
            Some(Placement::new(
                Rect::new(5.0, 5.0, 105.0, 105.0),
                Rect::new(10.0, 20.0, 30.0, 60.0),
            ))
        );
        let handler = frame.handler.unwrap();
        let mut log = vec![];
        assert!(handler.dispatch_pointer_down(&mut log, &pointer_at(15.0, 25.0)));
        assert!(!handler.dispatch_pointer_down(&mut log, &pointer_at(35.0, 25.0)));
        assert!(!handler.dispatch_pointer_down(&mut log, &pointer_at(15.0, 65.0)));
        for (x, handled) in [(15.0, true), (35.0, false)] {
            let pointer = pointer_at(x, 25.0);
            assert_eq!(
                handler
                    .dispatch_scroll(
                        &mut log,
                        &PointerScrollEvent {
                            pointer: pointer.pointer,
                            state: pointer.state,
                            delta: ScrollDelta::LineDelta(0.0, 1.0),
                        }
                    )
                    .handled(),
                handled
            );
        }
        assert!(handler.dispatch_pointer_up(&mut log, &pointer_at(200.0, 200.0)));
        assert!(handler.dispatch_key(&mut log, &Default::default()));
        assert_eq!(log, ["down", "scroll", "up", "key"]);
        let mut drawing = DrawList::new();
        for render in frame.renders {
            render(&mut drawing, Default::default());
        }
        assert!(
            matches!(&drawing.0[..], [DrawCmd::Clip { shape: Shape::Rect(rect), children, .. }]
            if *rect == viewport.rect && matches!(&children[..], [DrawCmd::Fill { .. }]))
        );
    }

    #[test]
    fn floaters_are_out_of_flow_and_only_place_when_requested() {
        let seen = Rc::new(RefCell::new(vec![]));
        for show in [false, true] {
            let capture = seen.clone();
            let base = leaf(Extent::default(), |_: &mut Frame, _| {});
            let child = leaf(extent(), move |_: &mut Frame, placement| {
                capture.borrow_mut().push(placement)
            });
            let overlay = Placement::root(Rect::new(20.0, 30.0, 120.0, 130.0));
            let widget = floating(base, child, move |_, extent| {
                assert_eq!(extent.width, 100.0);
                show.then_some(overlay)
            });
            assert_eq!(widget.extent, Extent::default());
            let output = measured::place(widget, Placement::root(Rect::ZERO));
            assert_eq!(output.floaters.len(), usize::from(show));
        }
        assert_eq!(
            &*seen.borrow(),
            &[Placement::root(Rect::new(20.0, 30.0, 120.0, 130.0))]
        );
    }
}
