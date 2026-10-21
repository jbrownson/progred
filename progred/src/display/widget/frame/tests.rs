use super::super::leaf;
use super::*;
use puri::handler::test_support::DispatchShorthands;
use std::{cell::RefCell, rc::Rc};

#[test]
fn view_scope_hides_other_views_hover_without_hiding_input() {
    use puri::handler::{Event, EventOutcome};
    use ui_events::pointer::{PointerState, PointerUpdate};
    let own = Root::document();
    let other = Root::pane(vec![gid::Step::Key(gid::new_cell_id())]);
    let mut pass: HoverPass<Vec<(u8, Option<u8>)>, u8> =
        HoverPass::new(&Default::default(), Rect::ZERO);
    for (id, root) in [(1, own.clone()), (2, other)] {
        pass.in_view(root, |pass| {
            pass.visit(|output| {
                output.handler().on(move |log, event, input| {
                    log.push((id, input.hovered().copied()));
                    EventOutcome::decline(event)
                });
                output.after_hover(move |_, output| {
                    output.handler().on(move |log, event, input| {
                        log.push((id + 10, input.hovered().copied()));
                        EventOutcome::decline(event)
                    });
                });
            })
        });
    }
    let handler = pass.finish().bind(Default::default()).handler.unwrap();
    let update = PointerUpdate {
        pointer: crate::display::test_support::mouse(),
        current: PointerState::default(),
        coalesced: vec![],
        predicted: vec![],
    };
    let mut input = DispatchContext::new(Some(own), Some(7));
    let mut log = Vec::new();
    handler.dispatch(&mut log, Event::PointerMove(&update), &mut input);
    log.sort_by_key(|entry| entry.0);
    assert_eq!(log, [(1, Some(7)), (2, None), (11, Some(7)), (12, None)]);
    assert_eq!(input.hovered(), Some(&7));
}

#[test]
fn hover_runs_in_paint_order_and_handlers_run_front_to_back() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let placement = Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0));
    let widget = |id| {
        let calls = calls.clone();
        leaf(
            measured::Extent {
                width: 10.0,
                ascent: 10.0,
                descent: 0.0,
            },
            move |output: &mut HoverContext<'_, Vec<u8>, u8>, placement| {
                calls.borrow_mut().push(id);
                let log = calls.clone();
                output.claim(Probe::dynamic(placement, move |_| {
                    log.borrow_mut().push(id + 10);
                    Some(id)
                }));
                output.after_hover(move |hover, effects| {
                    assert_eq!(hover.hovered, Some(2));
                    calls.borrow_mut().push(id + 20);
                    effects
                        .renders
                        .push(Box::new(move |_| calls.borrow_mut().push(id + 30)));
                    effects.handler().on_key(move |log, _| {
                        log.push(id);
                        false
                    });
                });
            },
        )
    };
    let layout = measured::layers(vec![widget(1), widget(2)]);
    assert!(calls.borrow().is_empty());
    let output = place(
        layout,
        placement,
        &HoverInput {
            pointer: Some(placement.rect.center()),
            ..Default::default()
        },
    );
    assert_eq!(&*calls.borrow(), &[1, 11, 2, 12]);
    assert_eq!(output.claim, Some((None, Claim::Direct(2))));
    let output = output.bind(Attribution {
        hovered: Some(2),
        ..Default::default()
    });
    assert_eq!(&*calls.borrow(), &[1, 11, 2, 12, 21, 22]);
    puri::frame::render(output.renders, &mut puri::draw::DrawList::new());
    assert_eq!(&*calls.borrow(), &[1, 11, 2, 12, 21, 22, 31, 32]);
    let mut log = Vec::new();
    output
        .handler
        .unwrap()
        .dispatch_key(&mut log, &Default::default());
    assert_eq!(log, [2, 1]);
}

#[test]
fn a_frame_can_discard_paint_and_still_dispatch() {
    let mut pass = HoverPass::<usize, ()>::new(&Default::default(), Rect::ZERO);
    pass.visit(|output| {
        output.render(|_, _| panic!("paint is optional"));
        output.after_hover(|_, effects| {
            effects.handler().on_key(|count, _| {
                *count += 1;
                true
            });
        });
    });
    let frame = pass.finish().bind(Default::default());
    drop(frame.renders);
    let mut count = 0;
    assert!(
        frame
            .handler
            .unwrap()
            .dispatch_key(&mut count, &Default::default())
    );
    assert_eq!(count, 1);
}

#[test]
fn nested_floaters_follow_their_parent_before_later_siblings() {
    fn mark(pass: &mut HoverPass<Vec<u8>, u8>, id: u8) {
        pass.visit(move |output| {
            output.claim(Probe::exact(
                Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)),
                id,
            ));
            output.handler().on_key(move |log, _| {
                log.push(id);
                false
            });
        });
    }
    let mut pass = HoverPass::new(
        &HoverInput {
            pointer: Some(Point::new(5.0, 5.0)),
            ..Default::default()
        },
        Rect::ZERO,
    );
    pass.float(|pass| {
        mark(pass, 1);
        pass.float(|pass| mark(pass, 2));
    });
    pass.float(|pass| mark(pass, 3));
    mark(&mut pass, 0);
    let output = pass.finish();
    assert_eq!(output.claim, Some((None, Claim::Direct(3))));
    let output = output.bind(Attribution {
        hovered: Some(3),
        ..Default::default()
    });
    let mut log = Vec::new();
    output
        .handler
        .unwrap()
        .dispatch_key(&mut log, &Default::default());
    assert_eq!(log, [3, 2, 1, 0]);
}

#[test]
fn retained_probes_preserve_order_clipping_retention_shapes_and_view_ownership() {
    let root = Root::document();
    let front = Root::document();
    let layout = |pass: &mut HoverPass<(), u8>| {
        pass.in_view(root.clone(), |pass| {
            pass.visit(|output| {
                output.claim(Probe::retaining(
                    Placement::new(
                        Rect::new(0.0, 0.0, 10.0, 10.0),
                        Rect::new(0.0, 0.0, 50.0, 20.0),
                    ),
                    1,
                ));
                output.claim(Probe::dynamic(
                    Placement::new(
                        Rect::new(20.0, 0.0, 40.0, 20.0),
                        Rect::new(20.0, 0.0, 35.0, 20.0),
                    ),
                    |point| (point.distance(Point::new(30.0, 10.0)) < 8.0).then_some(2),
                ));
            });
        });
        let front = front.clone();
        pass.float(move |pass| {
            pass.in_view(front, |pass| {
                pass.visit(|output| {
                    output.claim(Probe::occludes(Placement::root(Rect::new(
                        0.0, 0.0, 5.0, 5.0,
                    ))));
                    output.claim(Probe::exact(
                        Placement::root(Rect::new(1.0, 1.0, 3.0, 3.0)),
                        3,
                    ));
                });
            });
        });
    };
    let mut pass = HoverPass::new(&Default::default(), Rect::ZERO);
    layout(&mut pass);
    let retained = pass.finish().bind(Default::default()).hover_geometry;
    for (point, prior, expected) in [
        (
            Point::new(2.0, 2.0),
            None,
            Some((Some(front.clone()), Claim::Direct(3))),
        ),
        (
            Point::new(4.0, 4.0),
            Some(1),
            Some((Some(front.clone()), Claim::Occludes)),
        ),
        (
            Point::new(8.0, 8.0),
            None,
            Some((Some(root.clone()), Claim::Direct(1))),
        ),
        (
            Point::new(12.0, 8.0),
            Some(1),
            Some((Some(root.clone()), Claim::Extended(1))),
        ),
        (Point::new(12.0, 8.0), None, None),
        (
            Point::new(30.0, 10.0),
            None,
            Some((Some(root.clone()), Claim::Direct(2))),
        ),
        (Point::new(21.0, 1.0), None, None),
        (Point::new(36.0, 10.0), None, None),
    ] {
        let input = HoverInput {
            pointer: Some(point),
            prior: prior.as_ref(),
            reach_px: 4.0,
            ..Default::default()
        };
        let mut pass = HoverPass::new(&input, Rect::ZERO);
        layout(&mut pass);
        assert_eq!(pass.finish().claim, expected);
        assert_eq!(
            retained.probe(input.pointer, input.prior, input.reach_px),
            expected
        );
    }
    assert_eq!(retained.probe(None, None, 0.0), None);
}
