use super::{Fragment, Place, before};
use crate::{ActionHandler, Layout};
use gid::Value;
use puri::handler::{HasHandler, PointerButtonEvent};
use puri::{Placement, Point};
use std::rc::Rc;

pub fn click<World: 'static, Hover: 'static>(
    handler: ActionHandler<World>,
    primary: fn(&PointerButtonEvent) -> bool,
) -> Place<World, Hover> {
    Box::new(move |output, placement| {
        output.handler().on_pointer_down(move |world, event| {
            primary(event)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && handler(world)
        });
    })
}

pub fn target_action<World: 'static, Hover: 'static>(
    target: Hover,
    handler: ActionHandler<World>,
    pick: bool,
    picking: fn(&PointerButtonEvent) -> bool,
    same_target: fn(&Hover, &Hover) -> bool,
) -> Place<World, Hover> {
    Box::new(
        move |output: &mut Fragment<World, Hover>, placement: Placement| {
            if !placement.clipped_out() {
                output
                    .handler()
                    .on_pointer_down_with(move |world, event, hovered| {
                        puri::interact::is_primary_contact(event)
                            && picking(event) == pick
                            && hovered
                                .as_ref()
                                .is_some_and(|hover| same_target(hover, &target))
                            && handler(world)
                    });
            }
        },
    )
}

pub fn on_click<World: 'static, Hover: 'static>(
    child: Layout<World, Hover>,
    handler: ActionHandler<World>,
) -> Layout<World, Hover> {
    before(
        child,
        Rc::new(move |context| click(handler.clone(), context.primary_edit)),
    )
}

pub fn on_activate<World: 'static, Hover: Clone + 'static>(
    child: Layout<World, Hover>,
    target: Hover,
    handler: ActionHandler<World>,
) -> Layout<World, Hover> {
    before(
        child,
        Rc::new(move |context| {
            target_action(
                target.clone(),
                handler.clone(),
                false,
                context.picking,
                context.same_target,
            )
        }),
    )
}

pub fn pickable<World: 'static, Hover: Clone + 'static>(
    child: Layout<World, Hover>,
    target: Hover,
    value: Value,
) -> Layout<World, Hover> {
    before(
        child,
        Rc::new(move |context| {
            let pick = context.pick.clone();
            let value = value.clone();
            target_action(
                target.clone(),
                Rc::new(move |world| pick(world, value.clone())),
                true,
                context.picking,
                context.same_target,
            )
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use measured::{Extent, Output};
    use puri::Rect;
    use puri::handler::{PointerButton, PointerInfo, PointerState, PointerType};

    fn press(x: f64) -> PointerButtonEvent {
        PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: PointerInfo {
                pointer_id: None,
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state: PointerState {
                position: (x, 5.0).into(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn raw_click_uses_the_live_world_and_both_placement_rectangles() {
        let mut output = Fragment::<usize, ()>::empty();
        click(
            Rc::new(|world| {
                *world += 1;
                true
            }),
            puri::interact::is_primary_contact,
        )(
            &mut output,
            Placement::new(
                Rect::new(0.0, 0.0, 20.0, 20.0),
                Rect::new(0.0, 0.0, 10.0, 20.0),
            ),
        );
        let handler = output.handler.unwrap();
        let mut world = 10;
        for (x, expected) in [
            (-1.0, false),
            (5.0, true),
            (15.0, false),
            (25.0, false),
            (5.0, true),
        ] {
            assert_eq!(
                handler.dispatch_pointer_down(&mut world, &press(x)),
                expected
            );
        }
        assert_eq!(world, 12);
    }

    #[test]
    fn semantic_actions_use_settled_hover_not_a_second_hit_test() {
        for pick in [false, true] {
            for picking in [false, true] {
                let mut output = Fragment::<usize, u32>::empty();
                target_action(
                    7,
                    Rc::new(|world| {
                        *world += 1;
                        true
                    }),
                    pick,
                    if picking { |_| true } else { |_| false },
                    PartialEq::eq,
                )(
                    &mut output,
                    Placement::root(Rect::new(0.0, 0.0, 20.0, 20.0)),
                );
                let handler = output.handler.unwrap();
                let mut world = 0;
                for (mut hovered, expected) in
                    [(None, false), (Some(8), false), (Some(7), pick == picking)]
                {
                    assert_eq!(
                        handler.dispatch_pointer_down_with(&mut world, &press(25.0), &mut hovered),
                        expected
                    );
                }
                let mut secondary = press(5.0);
                secondary.button = Some(PointerButton::Secondary);
                assert!(!handler.dispatch_pointer_down_with(&mut world, &secondary, &mut Some(7)));
                assert_eq!(world, usize::from(pick == picking));
            }
        }
        let mut clipped = Fragment::<usize, u32>::empty();
        target_action(
            7,
            Rc::new(|_| panic!("clipped action")),
            false,
            |_| false,
            PartialEq::eq,
        )(
            &mut clipped,
            Placement::new(
                Rect::new(0.0, 0.0, 20.0, 20.0),
                Rect::new(30.0, 0.0, 40.0, 20.0),
            ),
        );
        assert!(clipped.handler.is_none());
    }

    #[test]
    fn leading_handlers_run_after_children_and_receive_declined_input() {
        for accepts in [false, true] {
            let place = click(
                Rc::new(|world: &mut Vec<&str>| {
                    world.push("outer");
                    true
                }),
                |_| true,
            );
            let child = super::super::leaf::<Vec<&str>, ()>(
                Extent {
                    width: 20.0,
                    ascent: 10.0,
                    descent: 10.0,
                },
                move |output, _| {
                    output.handler().on_pointer_down(move |world, _| {
                        world.push("child");
                        accepts
                    });
                },
            );
            let measured =
                measured::before_into(child, move |placement, output| place(output, placement));
            let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
            let output = measured::place(measured, placement);
            let mut world = vec![];
            assert!(
                output
                    .handler
                    .unwrap()
                    .dispatch_pointer_down(&mut world, &press(5.0))
            );
            assert_eq!(
                world,
                if accepts {
                    vec!["child"]
                } else {
                    vec!["child", "outer"]
                }
            );
        }
    }
}
