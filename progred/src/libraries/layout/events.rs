//! Grap's ordinary event adapter: when a drawing's handler gets an event.
//! What it gets is the core's [`crate::site::event`] record. Native widgets
//! use Puri handlers directly.

use crate::display::host::{Host, Layout};
use crate::display::widget;
use crate::site::event as grap_event;
use puri::Point;
use puri::handler::{Event, EventOutcome, HasHandler};
use std::rc::Rc;

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
                    if let Event::Scroll(events) = event {
                        return puri::scroll::filter(
                            events,
                            |event| {
                                placement.contains(Point::new(
                                    event.state.position.x,
                                    event.state.position.y,
                                ))
                            },
                            |events| {
                                let value = grap_event::scroll(placement, scale, &events, command);
                                let handled = site
                                    .open(&mut world.session())
                                    .run_handler(function.clone(), value);
                                EventOutcome::from_handled(Event::Scroll(events), handled)
                            },
                        );
                    }
                    if let Event::Gesture(events) = event {
                        return puri::gesture::filter(
                            events,
                            |event| {
                                placement.contains(Point::new(
                                    event.state.position.x,
                                    event.state.position.y,
                                ))
                            },
                            |events| {
                                let value = grap_event::gesture(placement, scale, &events, command);
                                let handled = site
                                    .open(&mut world.session())
                                    .run_handler(function.clone(), value);
                                EventOutcome::from_handled(Event::Gesture(events), handled)
                            },
                        );
                    }
                    let point = match &event {
                        Event::PointerDown(event) => Some(event.state.position),
                        _ => None,
                    };
                    let handled = if point
                        .is_some_and(|point| !placement.contains(Point::new(point.x, point.y)))
                    {
                        false
                    } else {
                        let value = grap_event::value(&event, placement, scale, command);
                        site.open(&mut world.session())
                            .run_handler(function.clone(), value)
                    };
                    EventOutcome::from_handled(event, handled)
                });
            })
        }),
    )
}

#[cfg(test)]
mod tests;
