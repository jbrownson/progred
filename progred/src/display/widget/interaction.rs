use super::{HoverCallback, before};
use crate::display::widget::HoverContext;
use crate::display::{Action, Host, Layout};
use gid::Value;
use puri::handler::HasHandler;
use puri::{Placement, Point};
use std::rc::Rc;

/// What a press runs on the world it's dispatched to.
pub type WorldHandler<World> = Rc<dyn Fn(&mut World) -> bool>;

/// `action`, run through the session the world lends.
fn lent<World: Host>(action: Action) -> WorldHandler<World> {
    Rc::new(move |world| action(&mut world.session()))
}

/// Run `handler` on a press inside the placement that `primary` accepts,
/// given the frame's command key.
pub fn click<World: 'static, Hover: 'static>(
    handler: WorldHandler<World>,
    primary: crate::modifiers::PressRule,
) -> HoverCallback<World, Hover> {
    Box::new(
        move |output: &mut HoverContext<'_, World, Hover>, placement| {
            let primary = primary(output.input.command_modifier);
            output.handler().on_pointer_down(move |world, event| {
                primary(event)
                    && placement
                        .contains(Point::new(event.state.position.x, event.state.position.y))
                    && handler(world)
            });
        },
    )
}

/// Run `handler` on a primary press over `target`, when the frame's command
/// key is held exactly if `pick`.
pub fn target_action<World: 'static, Hover: 'static>(
    target: Hover,
    handler: WorldHandler<World>,
    pick: bool,
    same_target: fn(&Hover, &Hover) -> bool,
) -> HoverCallback<World, Hover> {
    Box::new(
        move |output: &mut HoverContext<'_, World, Hover>, placement: Placement| {
            let picking = crate::modifiers::picking(output.input.command_modifier);
            if !placement.clipped_out() {
                output
                    .handler()
                    .on_pointer_down_with(move |world, event, hovered| {
                        puri::interact::is_primary_contact(event)
                            && picking(event) == pick
                            && hovered
                                .hovered()
                                .is_some_and(|hover| same_target(hover, &target))
                            && handler(world)
                    });
            }
        },
    )
}

pub fn on_click<World: Host, Hover: 'static>(
    child: Layout<World, Hover>,
    action: Action,
) -> Layout<World, Hover> {
    let handler = lent(action);
    before(
        child,
        Rc::new(move |_| click(handler.clone(), crate::modifiers::primary_edit)),
    )
}

pub fn on_activate<World: Host, Hover: Clone + PartialEq + 'static>(
    child: Layout<World, Hover>,
    target: Hover,
    action: Action,
) -> Layout<World, Hover> {
    let handler = lent(action);
    before(
        child,
        Rc::new(move |_| target_action(target.clone(), handler.clone(), false, PartialEq::eq)),
    )
}

pub fn pickable<W: Host>(
    child: crate::display::host::Layout<W>,
    target: W::Hover,
    value: Value,
) -> crate::display::host::Layout<W> {
    pickable_runtime(child, target, value.into())
}

pub fn pickable_runtime<W: Host>(
    child: crate::display::host::Layout<W>,
    target: W::Hover,
    value: grap::RuntimeValue,
) -> crate::display::host::Layout<W> {
    before(
        child,
        Rc::new(move |_| {
            let value = value.clone();
            target_action(
                target.clone(),
                Rc::new(move |world: &mut W| {
                    world
                        .session()
                        .pending()
                        .is_some_and(|picking| picking.pick(value.to_value()))
                }),
                true,
                PartialEq::eq,
            )
        }),
    )
}

#[cfg(test)]
mod tests;
