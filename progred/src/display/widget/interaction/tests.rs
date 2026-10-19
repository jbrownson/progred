use super::*;
use crate::display::recording::{Recorded, record};
use crate::display::test_support::press_at;
use crate::display::widget::HoverContext;
use crate::display::widget::frame::HoverOutput;
use kurbo::{Point, Rect};
use measured::Extent;
use puri::handler::HasHandler;
use puri::handler::test_support::DispatchShorthands;
use ui_events::keyboard::Modifiers;
use ui_events::pointer::PointerButton;
use uig::Placement;

#[test]
fn presses_use_settled_hover_not_a_second_hit_test() {
    for picking in [false, true] {
        let mut frame = HoverOutput::default();
        let mut navigation = Default::default();
        HoverContext::<(usize, usize), u32>::new(Default::default(), &mut frame, &mut navigation)
            .on_press(
                7,
                |(activated, _), _| {
                    *activated += 1;
                    true
                },
                |(_, picked), _| {
                    *picked += 1;
                    true
                },
            );
        let handler = frame.handler.unwrap();
        let mut world = (0, 0);
        // Holding the frame's command key is what makes a press pick.
        let mut event = press_at(Point::new(25.0, 5.0), Modifiers::empty());
        if picking {
            event.state.modifiers = crate::test_support::command();
        }
        for (hovered, expected) in [(None, false), (Some(8), false), (Some(7), true)] {
            assert_eq!(
                handler.dispatch_pointer_down_with(
                    &mut world,
                    &event,
                    &mut crate::display::widget::frame::DispatchContext::new(None, hovered)
                ),
                expected
            );
        }
        let mut secondary = press_at(Point::new(5.0, 5.0), Modifiers::empty());
        secondary.button = Some(PointerButton::Secondary);
        assert!(!handler.dispatch_pointer_down_with(
            &mut world,
            &secondary,
            &mut crate::display::widget::frame::DispatchContext::new(None, Some(7))
        ));
        assert_eq!(world, if picking { (0, 1) } else { (1, 0) });
    }
}

#[test]
fn clipped_out_presses_install_nothing() {
    crate::display::test_support::with_context::<crate::Editor, crate::frame::Hovered, _>(
        &crate::display::test_support::NoProject,
        |context| {
            let clipped = Placement::new(
                Rect::new(0.0, 0.0, 20.0, 20.0),
                Rect::new(30.0, 0.0, 40.0, 20.0),
            );
            for layout in [
                on_activate(
                    crate::display::test_support::text("activate"),
                    crate::libraries::test_widgets::hover(vec![]),
                    Rc::new(|_| panic!("clipped action")),
                ),
                pickable(
                    crate::display::test_support::text("pick"),
                    crate::libraries::test_widgets::hover(vec![]),
                    gid::Value::record([]),
                ),
            ] {
                let Recorded::Before { before, .. } = record(&layout) else {
                    panic!("leading callback");
                };
                let mut output = HoverOutput::default();
                before(context)(
                    &mut HoverContext::new(
                        Default::default(),
                        &mut output,
                        &mut Default::default(),
                    ),
                    clipped,
                );
                assert!(output.handler.is_none());
            }
        },
    );
}

#[test]
fn leading_handlers_run_after_children_and_receive_declined_input() {
    for accepts in [false, true] {
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
        let measured = crate::display::widget::before_place(child, |_, output| {
            output
                .handler()
                .on_pointer_down(|world: &mut Vec<&str>, _| {
                    world.push("outer");
                    true
                });
        });
        let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
        let output = crate::display::widget::frame::place(measured, placement, &Default::default());
        let output = output.bind(Default::default());
        let mut world = vec![];
        assert!(output.handler.unwrap().dispatch_pointer_down(
            &mut world,
            &press_at(Point::new(5.0, 5.0), Modifiers::empty())
        ));
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
