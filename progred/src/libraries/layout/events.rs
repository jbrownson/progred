//! Grap's ordinary event adapter: when a drawing's handler gets an event.
//! What it gets is the core's [`crate::site::event`] record. Native widgets
//! use Puri handlers directly.

use crate::display::host::{Host, Layout};
use crate::display::widget;
use crate::site::event as grap_event;
use kurbo::Point;
use puri::handler::{Event, EventOutcome, HasHandler};
use std::rc::Rc;
use ui_events::pointer::PointerState;

pub fn on_event<W: Host>(child: Layout<W>, function: impl Into<::grap::RuntimeValue>) -> Layout<W> {
    let function = function.into();
    widget::before(
        child,
        Rc::new(move |context| {
            let site = context.site();
            let scale = context.viewing().scale();
            let function = function.clone();
            Box::new(move |output, placement| {
                let command = output.input.command_modifier;
                output.handler().on(move |world, event, _| {
                    let within = |pointer: &PointerState| {
                        placement.contains(Point::new(pointer.position.x, pointer.position.y))
                    };
                    let mut run = |value| {
                        site.open(&mut world.session())
                            .run_handler(function.clone(), value)
                    };
                    match event {
                        Event::Scroll(events) => puri::scroll::filter(
                            events,
                            |event| within(&event.state),
                            |events| {
                                let handled =
                                    run(grap_event::scroll(placement, scale, &events, command));
                                EventOutcome::from_handled(Event::Scroll(events), handled)
                            },
                        ),
                        Event::Gesture(events) => puri::gesture::filter(
                            events,
                            |event| within(&event.state),
                            |events| {
                                let handled =
                                    run(grap_event::gesture(placement, scale, &events, command));
                                EventOutcome::from_handled(Event::Gesture(events), handled)
                            },
                        ),
                        event => {
                            let outside = matches!(
                                &event,
                                Event::PointerDown(down) if !within(&down.state)
                            );
                            let handled = !outside
                                && run(grap_event::value(&event, placement, scale, command));
                            EventOutcome::from_handled(event, handled)
                        }
                    }
                });
            })
        }),
    )
}

#[cfg(test)]
#[path = "tests/events.rs"]
mod tests;
