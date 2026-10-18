use super::*;
use kurbo::Rect;
use measured::Extent;
use ui_events::pointer::{PointerButton, PointerButtonEvent, PointerState};

fn press(x: f64) -> PointerButtonEvent {
    PointerButtonEvent {
        button: Some(PointerButton::Primary),
        pointer: crate::display::test_support::mouse(),
        state: PointerState {
            position: (x, 5.0).into(),
            ..Default::default()
        },
    }
}

#[test]
fn raw_click_uses_the_live_world_and_both_placement_rectangles() {
    let mut frame = crate::display::widget::HoverOutput::default();
    let mut navigation = Default::default();
    let mut output = crate::display::widget::HoverContext::<usize, ()>::new(
        Default::default(),
        &mut frame,
        &mut navigation,
    );
    click(
        Rc::new(|world| {
            *world += 1;
            true
        }),
        |_| puri::interact::is_primary_contact,
    )(
        &mut output,
        Placement::new(
            Rect::new(0.0, 0.0, 20.0, 20.0),
            Rect::new(0.0, 0.0, 10.0, 20.0),
        ),
    );
    let handler = frame.handler.unwrap();
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
            let mut frame = crate::display::widget::HoverOutput::default();
            let mut navigation = Default::default();
            let mut output = crate::display::widget::HoverContext::<usize, u32>::new(
                Default::default(),
                &mut frame,
                &mut navigation,
            );
            target_action(
                7,
                Rc::new(|world| {
                    *world += 1;
                    true
                }),
                pick,
                PartialEq::eq,
            )(
                &mut output,
                Placement::root(Rect::new(0.0, 0.0, 20.0, 20.0)),
            );
            let handler = frame.handler.unwrap();
            let mut world = 0;
            // Holding the frame's command key is what makes a press pick.
            let mut event = press(25.0);
            if picking {
                event.state.modifiers = crate::modifiers::command();
            }
            for (hovered, expected) in [(None, false), (Some(8), false), (Some(7), pick == picking)]
            {
                assert_eq!(
                    handler.dispatch_pointer_down_with(
                        &mut world,
                        &event,
                        &mut crate::display::widget::frame::DispatchContext::new(None, hovered)
                    ),
                    expected
                );
            }
            let mut secondary = press(5.0);
            secondary.button = Some(PointerButton::Secondary);
            assert!(!handler.dispatch_pointer_down_with(
                &mut world,
                &secondary,
                &mut crate::display::widget::frame::DispatchContext::new(None, Some(7))
            ));
            assert_eq!(world, usize::from(pick == picking));
        }
    }
    let mut frame = crate::display::widget::HoverOutput::default();
    let mut navigation = Default::default();
    let mut clipped = crate::display::widget::HoverContext::<usize, u32>::new(
        Default::default(),
        &mut frame,
        &mut navigation,
    );
    target_action(
        7,
        Rc::new(|_| panic!("clipped action")),
        false,
        PartialEq::eq,
    )(
        &mut clipped,
        Placement::new(
            Rect::new(0.0, 0.0, 20.0, 20.0),
            Rect::new(30.0, 0.0, 40.0, 20.0),
        ),
    );
    assert!(frame.handler.is_none());
}

#[test]
fn leading_handlers_run_after_children_and_receive_declined_input() {
    for accepts in [false, true] {
        let place = click(
            Rc::new(|world: &mut Vec<&str>| {
                world.push("outer");
                true
            }),
            |_| |_| true,
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
        let measured = crate::display::widget::before_place(child, move |placement, output| {
            place(output, placement)
        });
        let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
        let output = crate::display::widget::frame::place(measured, placement, &Default::default());
        let output = output.bind(Default::default());
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
