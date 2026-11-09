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

/// Picks `value` into the open pending, if there's one.
pub(crate) fn pick<W: Host>(world: &mut W, value: &grap::RuntimeValue) -> bool {
    world
        .session()
        .pending()
        .is_some_and(|picking| picking.pick(value.to_value()))
}

#[cfg(test)]
#[path = "tests/interaction.rs"]
mod tests;
