use super::before;
use crate::display::{Action, Host, Layout};
use std::rc::Rc;

pub fn on_activate<World: Host, Hover: Clone + PartialEq + 'static>(
    child: Layout<World, Hover>,
    target: Hover,
    action: Action,
) -> Layout<World, Hover> {
    before(
        child,
        Rc::new(move |_| {
            let target = target.clone();
            let action = action.clone();
            Box::new(move |output, placement| {
                if !placement.clipped_out() {
                    output.on_press(
                        target,
                        move |world: &mut World, _| action(&mut world.session()),
                        |_, _| false,
                    );
                }
            })
        }),
    )
}

pub fn pickable<W: Host>(
    child: crate::display::host::Layout<W>,
    target: W::Hover,
    value: impl Into<grap::RuntimeValue>,
) -> crate::display::host::Layout<W> {
    let value = value.into();
    before(
        child,
        Rc::new(move |_| {
            let target = target.clone();
            let value = value.clone();
            Box::new(move |output, placement| {
                if !placement.clipped_out() {
                    output.on_press(
                        target,
                        |_, _| false,
                        move |world: &mut W, _| pick(world, &value),
                    );
                }
            })
        }),
    )
}

/// `child`, which a click on `target` activates with `action`, if it has one,
/// and a Cmd+click picks `value` into the open pending, or activates where
/// nothing is pending, so a stray modifier never deadens the click.
pub fn activate_or_pick<W: Host>(
    child: crate::display::host::Layout<W>,
    target: W::Hover,
    action: Option<Action>,
    value: impl Into<grap::RuntimeValue>,
) -> crate::display::host::Layout<W> {
    let value = value.into();
    before(
        child,
        Rc::new(move |_| {
            let target = target.clone();
            let action = action.clone();
            let value = value.clone();
            Box::new(move |output, placement| {
                if !placement.clipped_out() {
                    let activate = action.clone();
                    let action = action.clone();
                    let value = value.clone();
                    output.on_press(
                        target,
                        move |world: &mut W, _| {
                            activate
                                .as_ref()
                                .is_some_and(|activate| activate(&mut world.session()))
                        },
                        move |world: &mut W, _| {
                            pick(world, &value)
                                || action
                                    .as_ref()
                                    .is_some_and(|action| action(&mut world.session()))
                        },
                    );
                }
            })
        }),
    )
}

/// Picks `value` into the open pending, if there's one.
pub(crate) fn pick<W: Host>(world: &mut W, value: &grap::RuntimeValue) -> bool {
    world
        .session()
        .pending()
        .is_some_and(|picking| picking.pick(value.to_value()))
}

#[cfg(test)]
mod tests;
