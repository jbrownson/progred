use super::*;
use crate::display::recording::{Recorded, record};
use crate::frame::Hovered;
use gid::{Cells, Document};
use kurbo::{Affine, Point, Rect, Stroke};
use peniko::Color;
use puri::draw::{DrawCmd, DrawList, Shape};
use puri::handler::HasHandler;
use ui_events::keyboard::KeyboardEvent;

#[test]
fn fill_width_expands_ink_and_interaction_together_without_changing_measurement() {
    use crate::display::test_support::{NoProject, with_context};
    use measured::choices::{ChoiceBuild, resolve_choices};
    use puri::hover::Claim;

    let activated = crate::display::on_activate::<crate::Editor, u32>(
        Layout::widget(Rc::new(|_| {
            paint(
                Extent {
                    width: 0.0,
                    ascent: 0.0,
                    descent: 16.0,
                },
                |canvas, placement| {
                    canvas.fill_shape(placement.rect.into(), Color::BLACK.into(), Affine::IDENTITY)
                },
            )
        })),
        7_u32,
        Rc::new(|_| true),
    );
    let layout = fill_width(crate::display::on_hover(activated, 7_u32));
    let mut build = ChoiceBuild::default();
    let prepared = with_context(&NoProject, |context| layout.measure(context, &mut build));
    let measured = resolve_choices(build.finish(prepared), 100.0);
    assert_eq!(measured.extent.width, 0.0);
    assert_eq!(measured.extent.height(), 16.0);
    let output = frame::place(
        measured,
        Placement::new(
            Rect::new(10.0, 20.0, 10.0, 36.0),
            Rect::new(30.0, 0.0, 80.0, 100.0),
        )
        .with_available_rect(Rect::new(10.0, 20.0, 100.0, 36.0)),
        &HoverInput::default(),
    );
    for (point, expected) in [
        (Point::new(50.0, 28.0), Some(Claim::Direct(7))),
        (Point::new(20.0, 28.0), None),
        (Point::new(90.0, 28.0), None),
        (Point::new(50.0, 40.0), None),
    ] {
        assert_eq!(
            output
                .hover_geometry
                .probe(Some(point), None, 0.0)
                .map(|(_, claim)| claim),
            expected
        );
    }
    let mut drawing = DrawList::new();
    puri::frame::render(output.bind(Default::default()).renders, &mut drawing);
    assert!(
        matches!(&drawing.0[..], [DrawCmd::Fill { shape: Shape::Rect(rect), .. }]
        if *rect == Rect::new(10.0, 20.0, 100.0, 36.0))
    );
}

#[test]
fn paint_only_leaves_defer_ink_and_skip_clipped_or_discarded_paint() {
    let extent = Extent {
        width: 20.0,
        ascent: 8.0,
        descent: 2.0,
    };
    let rect = Rect::new(30.0, 40.0, 50.0, 50.0);
    for (clip, render) in [(rect, false), (rect, true), (Rect::ZERO, true)] {
        let calls = Rc::new(std::cell::Cell::new(0));
        let drawing = calls.clone();
        let leaf = paint::<(), ()>(extent, move |canvas, placement| {
            drawing.set(drawing.get() + 1);
            canvas.fill_shape(placement.rect.into(), Color::BLACK.into(), Affine::IDENTITY);
        });
        assert_eq!(leaf.extent, extent);
        let mut placement = Placement::root(rect);
        placement.clip_rect = clip;
        let fragment = crate::display::widget::frame::place(leaf, placement, &Default::default());
        assert_eq!(calls.get(), 0);
        assert!(fragment.claim.is_none());
        assert!(fragment.handler.is_none());
        let mut canvas = DrawList::new();
        if render {
            puri::frame::render(fragment.bind(Default::default()).renders, &mut canvas);
        }
        let painted = render && clip == rect;
        assert_eq!(calls.get(), usize::from(painted));
        assert_eq!(canvas.0.len(), usize::from(painted));
        if painted {
            assert!(
                matches!(&canvas.0[0], DrawCmd::Fill { shape: Shape::Rect(drawn), .. } if *drawn == rect)
            );
        }
    }
}

#[test]
fn placement_hover_render_and_dispatch_are_distinct_stages() {
    let calls = Rc::new(std::cell::RefCell::new(Vec::new()));
    let during_hover = calls.clone();
    let measured = leaf(
        Extent {
            width: 20.0,
            ascent: 8.0,
            descent: 2.0,
        },
        move |output: &mut HoverContext<'_, Vec<&str>, usize>, placement| {
            during_hover.borrow_mut().push("hover");
            output.claim(Probe::retaining(placement, 7));
            let during_render = during_hover.clone();
            output.render(move |canvas, hover| {
                during_render.borrow_mut().push("render");
                assert_eq!(hover.hovered, Some(7));
                canvas.fill_shape(placement.rect.into(), Color::BLACK.into(), Affine::IDENTITY);
            });
            output.handler().on_key(|state, _| {
                state.push("key");
                true
            });
            output.on_arrival(
                Rc::from([]),
                Rc::new(|state, _| {
                    state.push("select");
                    true
                }),
            );
        },
    );
    assert!(calls.borrow().is_empty());
    let placement = Placement::root(Rect::new(10.0, 20.0, 30.0, 30.0));
    let mut output = place(
        measured,
        placement,
        &HoverInput {
            pointer: Some(placement.rect.center()),
            ..Default::default()
        },
    );
    assert_eq!(&*calls.borrow(), &["hover"]);
    assert_eq!(
        output.claim.clone().map(|(_, claim)| claim),
        Some(puri::hover::Claim::Direct(7))
    );
    let mut canvas = DrawList::new();
    let (_, select) = output.landmark_select.take().unwrap();
    let output = output.bind(Attribution {
        hovered: Some(7),
        ..Default::default()
    });
    puri::frame::render(output.renders, &mut canvas);
    assert_eq!(&*calls.borrow(), &["hover", "render"]);
    assert!(
        matches!(&canvas.0[..], [DrawCmd::Fill { shape: Shape::Rect(rect), .. }] if *rect == placement.rect)
    );
    let mut state = vec![];
    assert!(
        output
            .handler
            .unwrap()
            .dispatch_key(&mut state, &ui_events::keyboard::KeyboardEvent::default())
    );
    assert!(select(&mut state, Some(Direction::Left)));
    assert_eq!(state, ["key", "select"]);
}

#[test]
fn native_canvas_clip_streams_to_the_selected_interpreter() {
    let mut frame = HoverOutput::default();
    let mut navigation = Default::default();
    let mut output: HoverContext<'_, (), ()> =
        HoverContext::new(Default::default(), &mut frame, &mut navigation);
    let clip = Rect::new(0.0, 0.0, 20.0, 10.0);
    output.render(move |canvas, _| {
        canvas.with_clip(
            clip.into(),
            Affine::IDENTITY,
            Box::new(move |canvas| {
                canvas.fill_shape(clip.into(), Color::BLACK.into(), Affine::IDENTITY);
                canvas.with_clip(
                    clip.into(),
                    Affine::translate((2.0, 0.0)),
                    Box::new(move |canvas| {
                        canvas.stroke_shape(
                            clip.into(),
                            Stroke::new(1.0),
                            Color::WHITE.into(),
                            Affine::IDENTITY,
                        );
                    }),
                );
            }),
        )
    });
    let output = frame;
    assert_eq!(output.after_hover.len(), 1);
    let mut canvas = DrawList::new();
    puri::frame::render(output.bind(Default::default()).renders, &mut canvas);
    let [DrawCmd::Clip { children, .. }] = &canvas.0[..] else {
        panic!("outer clip");
    };
    let [DrawCmd::Fill { .. }, DrawCmd::Clip { children, .. }] = &children[..] else {
        panic!("nested clip");
    };
    assert!(matches!(&children[..], [DrawCmd::Stroke { .. }]));
}

#[test]
fn render_is_one_continuation_regardless_of_drawing_count() {
    let calls = Rc::new(std::cell::Cell::new(0));
    let during_render = calls.clone();
    let mut frame = HoverOutput::default();
    let mut navigation = Default::default();
    let mut output = crate::display::widget::HoverContext::<(), ()>::new(
        Default::default(),
        &mut frame,
        &mut navigation,
    );
    output.render(move |canvas, _| {
        for _ in 0..100 {
            during_render.set(during_render.get() + 1);
            canvas.fill_shape(
                Rect::new(0.0, 0.0, 1.0, 1.0).into(),
                Color::BLACK.into(),
                Affine::IDENTITY,
            );
        }
    });
    assert_eq!(calls.get(), 0);
    let output = frame;
    assert_eq!(output.after_hover.len(), 1);
    let mut canvas = DrawList::new();
    puri::frame::render(output.bind(Default::default()).renders, &mut canvas);
    assert_eq!(calls.get(), 100);
    assert_eq!(canvas.0.len(), 100);
}

#[test]
fn inert_widgets_and_pointer_wrappers_do_not_request_site_state() {
    crate::display::test_support::with_context::<crate::Editor, crate::frame::Hovered, _>(
        &crate::display::test_support::NoProject,
        |context| {
            let measured = delimiter::side(
                crate::display::Delim::Bracket,
                crate::display::Side::Open,
            )(context);
            let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
            assert_eq!(
                crate::display::widget::frame::place(measured, placement, &Default::default())
                    .after_hover
                    .len(),
                1
            );
            for layout in [
                interaction::on_activate(
                    crate::display::text("activate"),
                    crate::libraries::test_widgets::hover(vec![]),
                    Rc::new(|_| true),
                ),
                interaction::pickable(
                    crate::display::text("pick"),
                    crate::libraries::test_widgets::hover(vec![]),
                    Value::record([]),
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
                    placement,
                );
                assert!(output.handler.is_some());
            }
            for layout in [
                hover::on_hover(
                    crate::display::text("claim"),
                    crate::libraries::test_widgets::hover(vec![]),
                ),
                hover::block_hover(crate::display::text("occluder")),
                hover::hover_highlight(
                    crate::display::text("highlight"),
                    crate::libraries::test_widgets::hover(vec![]),
                ),
            ] {
                let Recorded::Before { before, .. } = record(&layout) else {
                    panic!("leading callback");
                };
                before(context)(
                    &mut HoverContext::new(
                        Default::default(),
                        &mut HoverOutput::default(),
                        &mut Default::default(),
                    ),
                    placement,
                );
            }
            assert_eq!(
                crate::display::widget::frame::place(
                    empty::<(), ()>(context.text, context.toolkit.styles()),
                    placement,
                    &Default::default()
                )
                .after_hover
                .len(),
                1
            );
        },
    );
}

#[test]
fn discarded_layout_alternatives_do_not_place_widget_outputs() {
    use measured::choices::{ChoiceBuild, ChoiceLayout, resolve_choices};
    let widget = |width, target| {
        ChoiceLayout::fixed(leaf(
            Extent {
                width,
                ascent: 10.0,
                descent: 0.0,
            },
            move |output: &mut HoverContext<'_, (), usize>, placement| {
                output.claim(Probe::retaining(placement, target))
            },
        ))
    };
    let mut choices = ChoiceBuild::default();
    let layout = choices.alternatives(vec![widget(100.0, 1), widget(20.0, 2)]);
    let measured = resolve_choices(choices.finish(layout), 50.0);
    let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
    let output = crate::display::widget::frame::place(
        measured,
        placement,
        &HoverInput {
            pointer: Some(placement.rect.center()),
            ..Default::default()
        },
    );
    assert_eq!(
        output.claim.clone().map(|(_, claim)| claim),
        Some(puri::hover::Claim::Direct(2))
    );
}

#[test]
fn native_decorators_preserve_front_to_back_input_and_back_to_front_paint() {
    let log = Rc::new(std::cell::RefCell::new(vec![]));
    let contribution = |name: &'static str| {
        let log = log.clone();
        Box::new(
            move |output: &mut crate::display::widget::HoverContext<'_, crate::Editor, Hovered>,
                  _: Placement| {
                let during_paint = log.clone();
                output.render(move |_, _| during_paint.borrow_mut().push(name));
                output.handler().on_key(move |_, _| {
                    log.borrow_mut().push(name);
                    false
                });
            },
        ) as crate::display::widget::HoverCallback<crate::Editor, Hovered>
    };
    let extent = Extent {
        width: 40.0,
        ascent: 10.0,
        descent: 5.0,
    };
    let child = crate::display::widget::leaf(extent, contribution("child"));
    let before = contribution("before");
    let after = contribution("after");
    let decorated = crate::display::widget::after_place(
        crate::display::widget::before_place(child, move |placement, output| {
            before(output, placement)
        }),
        move |placement, output| after(output, placement),
    );
    assert_eq!(decorated.extent, extent);
    let output = {
        let layout = decorated;
        let placement = uig::Placement::root(layout.extent.rect_at(Point::ZERO));
        crate::display::widget::frame::place(layout, placement, &Default::default())
    };
    let output = output.bind(Default::default());
    assert!(log.borrow().is_empty());
    assert!(!output.handler.unwrap().dispatch_key(
        &mut crate::test_editor(Document {
            root: None,
            cells: Cells::new()
        }),
        &KeyboardEvent::default()
    ));
    assert_eq!(&*log.borrow(), &["after", "child", "before"]);
    log.borrow_mut().clear();
    puri::frame::render(output.renders, &mut DrawList::new());
    assert_eq!(&*log.borrow(), &["before", "child", "after"]);
}
