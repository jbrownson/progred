use super::*;
use crate::display::test_support::ResolveForDispatch;
use crate::display::test_support::primary_press;
use crate::display::widget::placed::leaf;
use crate::display::widget::view::Area;
use crate::libraries::f64 as f64_convention;
use crate::libraries::test_widgets::{drag_frame, gesture_place, hover};
use crate::selection::{Stage, pending_value};
use gid::{Step, new_cell_id};
use kurbo::Point;
use measured::Extent;
use puri::handler::HasHandler;
use puri::handler::test_support::DispatchShorthands;
use ui_events::keyboard::Modifiers;
use uig::Placement;

#[test]
fn scrub_start_respects_pending_selection_and_visible_view_geometry() {
    for (pending, covered, x, pick, owns_view, expected) in [
        (false, false, 5.0, true, true, true),
        (true, false, 5.0, true, true, false),
        (false, true, 5.0, true, true, false),
        (false, false, 15.0, true, true, false),
        (false, false, 5.0, false, true, false),
        (false, false, 5.0, true, false, false),
    ] {
        let frame = drag_frame(
            gesture_place(
                crate::libraries::number::scrub::on_scrub(
                    crate::display::row(0.0, []),
                    hover(vec![]),
                    Rc::new(|| {
                        Box::new(|_| crate::libraries::number::scrub::ScrubUpdate {
                            value: f64_convention::value(13.0),
                            spelling: "13".into(),
                        })
                    }),
                ),
                false,
            ),
            covered,
        );
        let mut world = crate::editor_of(f64_convention::value(1.0));
        if pending {
            world.model.selection = Some(pending_value(&crate::test_document(), vec![]));
        }
        let handled = frame.resolve_for_dispatch().dispatch_pointer_down_with(
            &mut world,
            &primary_press(
                Point::new(x, 5.0),
                if pick {
                    crate::test_support::command()
                } else {
                    Modifiers::empty()
                },
            ),
            &mut crate::frame::DispatchContext::new(
                owns_view.then(|| Area::whole(crate::test_document())),
                Some(hover(vec![])),
            ),
        );
        assert_eq!(handled, covered || expected);
        assert_eq!(world.gesture.is_some(), expected);
        if expected {
            world.advance_gesture(&[Point::new(50.0, 5.0)]);
            assert_eq!(world.model.doc.root, Some(f64_convention::value(13.0)));
            assert!(world.model.history.can_undo());
        }
    }
}

#[test]
fn scrub_declines_for_pending_pick_and_raw_contact_takes_precedence() {
    use crate::display::widget::before_place;

    for (pending, raw) in [(false, false), (true, false), (false, true), (true, true)] {
        let field = new_cell_id();
        let value = f64_convention::value(1.0);
        let mut world = crate::editor_of(value.clone());
        if pending {
            world.model.selection = Some(pending_value(
                &crate::test_document(),
                crate::occurrence::of(&[Step::Key(field)]),
            ));
        }
        let before = world.model.doc.clone();
        let raw_contacts = Rc::new(std::cell::Cell::new(0));
        let contacts = raw_contacts.clone();
        let scrub = gesture_place(
            crate::libraries::number::scrub::on_scrub(
                crate::display::row(0.0, []),
                hover(vec![]),
                Rc::new(|| {
                    Box::new(|_| crate::libraries::number::scrub::ScrubUpdate {
                        value: f64_convention::value(13.0),
                        spelling: "13".into(),
                    })
                }),
            ),
            false,
        );
        let extent = Extent {
            width: 20.0,
            ascent: 0.0,
            descent: 20.0,
        };
        let node = before_place(
            before_place(
                leaf(extent, move |p, _| {
                    p.handler().on_pointer_down(move |_, _| {
                        if raw {
                            contacts.set(contacts.get() + 1);
                        }
                        raw
                    });
                }),
                move |placement, output| scrub(output, placement),
            ),
            move |_, output| {
                output.on_press(
                    hover(vec![]),
                    |_, _| false,
                    move |world: &mut crate::Editor, _| {
                        world
                            .session()
                            .pending()
                            .is_some_and(|picking| picking.pick(value.clone()))
                    },
                )
            },
        );
        let frame = crate::display::widget::frame::place(
            crate::display::widget::view::in_view(node, Area::whole(crate::test_document())),
            Placement::root(extent.rect_at(Point::ZERO)),
            &Default::default(),
        );
        assert!(frame.resolve_for_dispatch().dispatch_pointer_down_with(
            &mut world,
            &primary_press(Point::new(5.0, 5.0), crate::test_support::command()),
            &mut crate::frame::DispatchContext::new(
                Some(Area::whole(crate::test_document())),
                Some(hover(vec![]))
            ),
        ));
        assert_eq!(raw_contacts.get(), usize::from(raw));
        assert_eq!(world.gesture.is_some(), !pending && !raw);
        if pending && !raw {
            assert_eq!(
                world.sources().resolve_path(&[Step::Key(field)]),
                Some(&f64_convention::value(1.0))
            );
            let selected = world.model.selection.as_ref().unwrap();
            assert_eq!(selected.path(), &[Step::Key(field)]);
            assert_eq!(selected.stage(&world.sources()), Stage::Edge);
            assert!(world.model.history.can_undo());
        } else {
            assert!(Rc::ptr_eq(&before, &world.model.doc));
        }
    }
}
