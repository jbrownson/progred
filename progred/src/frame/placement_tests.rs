use super::*;
use crate::display::test_support::{mouse, press_at};
use crate::display::widget::container::floating;
use crate::display::widget::container::scrolled;
use crate::display::widget::placed::leaf;
use crate::display::widget::view::in_view;
use crate::display::widget::{HoverContext, HoverInput};
use crate::projection::test_support::HoverOutput;
use kurbo::{Affine, Point, Rect, Stroke};
use measured::{Extent, Measured};
use peniko::{Brush, Color, ImageData};
use puri::draw::{Canvas, DrawCmd, DrawList, GlyphRun, Shape};
use puri::handler::HasHandler;
use ui_events::ScrollDelta;
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerScrollEvent, PointerState, PointerUpdate,
};
use uig::Placement;

type Placed<'a, 'input, C> = crate::display::widget::placed::Placed<'a, 'input, C, Hovered>;

struct TestCanvas(DrawList);

impl puri::draw::CanvasSink for TestCanvas {
    fn draw_image(&mut self, image: ImageData, transform: Affine) {
        self.0.draw_image(image, transform);
    }

    fn fill_shape(&mut self, shape: Shape, brush: Brush, transform: Affine) {
        self.0.fill(shape, brush, transform);
    }

    fn stroke_shape(&mut self, shape: Shape, style: Stroke, brush: Brush, transform: Affine) {
        self.0.stroke(shape, style, brush, transform);
    }

    fn draw_glyphs(&mut self, run: GlyphRun) {
        self.0.draw_glyphs(run);
    }

    fn with_clip(
        &mut self,
        shape: Shape,
        transform: Affine,
        content: Box<dyn FnOnce(&mut dyn puri::draw::CanvasSink) + '_>,
    ) {
        let mut inner = TestCanvas(DrawList::new());
        content(&mut inner);
        self.0.0.push(DrawCmd::Clip {
            shape,
            transform,
            children: inner.0.0,
        });
    }
}

fn no_ink() -> Attribution {
    Attribution {
        hovered: None,
        hovered_secondary: None,
        selected_secondary: None,
        hovered_trace: None,
    }
}

fn state_at(x: f64, y: f64) -> PointerState {
    let mut state = PointerState::default();
    state.position.x = x;
    state.position.y = y;
    state
}

fn native_delimiter(
    interactive: bool,
    scale: f64,
    span: Extent,
    side: puri::delim::Side,
) -> Measured<HoverPass<crate::Editor>> {
    use crate::display::widget;
    let value = gid::Value::record([]);
    let side = widget::delimiter::side(puri::delim::Delim::Bracket, side);
    let side = if interactive {
        widget::selectable_widget(side)
    } else {
        side
    };
    let native = crate::display::test_support::with_empty_view(
        &crate::display::test_support::NoProject,
        |cx, text, project| {
            let styles = crate::display::widget::style::editor(
                crate::display::widget::style::Theme::Light.palette(),
                scale,
            );
            let cx = crate::projection::Cx {
                styles: &styles,
                ..cx.clone()
            };
            widget::fill_height(side)(&mut widget::Context {
                value: Some(&value.into()),
                toolkit: &crate::projection::Occurrence { cx: &cx, path: &[] },
                project,
                text,
            })
        },
    );
    let extent = Extent {
        width: native.extent.width,
        ascent: span.ascent.max(native.extent.ascent),
        descent: span.descent.max(native.extent.descent),
    };
    measured::leaf_into(extent, move |placement, pass| {
        measured::place_into(native, placement, pass)
    })
}

#[test]
fn delimiter_interaction_is_opt_in_without_changing_ink_or_extent() {
    use kurbo::Shape as _;
    for scale in [1.0, 2.0] {
        for span in [
            Extent::default(),
            Extent {
                width: 40.0,
                ascent: 80.0,
                descent: 50.0,
            },
        ] {
            for side in [puri::delim::Side::Open, puri::delim::Side::Close] {
                let inert = native_delimiter(false, scale, span, side);
                let interactive = native_delimiter(true, scale, span, side);
                assert_eq!(inert.extent, interactive.extent);
                let placement = Placement::root(inert.extent.rect_at(Point::new(20.0, 30.0)));
                let inert =
                    crate::display::widget::frame::place(inert, placement, &Default::default());
                let interactive = crate::display::widget::frame::place(
                    interactive,
                    placement,
                    &HoverInput {
                        pointer: Some(placement.rect.center()),
                        ..Default::default()
                    },
                );
                assert!(inert.claim.is_none() && inert.handler.is_none());
                assert!(interactive.claim.is_some());
                assert_eq!(
                    interactive.claim.clone().map(|(_, claim)| claim),
                    Some(Claim::Direct(Hovered::Tree(crate::hover::Hover::Value(
                        std::rc::Rc::from([])
                    ))))
                );
                let outlines = [inert, interactive].map(|placed| {
                    let claimed = placed.claim.is_some();
                    let placed = placed.bind(no_ink());
                    let mut canvas = TestCanvas(DrawList::new());
                    puri::frame::render(placed.renders, &mut canvas);
                    assert_eq!(placed.handler.is_some(), claimed);
                    let [
                        DrawCmd::Fill {
                            shape: Shape::Path(path),
                            transform,
                            ..
                        },
                    ] = &canvas.0.0[..]
                    else {
                        panic!("delimiter outline")
                    };
                    let bounds = transform.transform_rect_bbox(path.bounding_box());
                    assert!(bounds.x0 >= placement.rect.x0 && bounds.x1 <= placement.rect.x1);
                    assert!((bounds.y0 - placement.rect.y0).abs() < 1e-6);
                    assert!((bounds.y1 - placement.rect.y1).abs() < 1e-6);
                    (path.clone(), *transform)
                });
                assert_eq!(outlines[0], outlines[1]);
            }
        }
    }
}

#[test]
fn native_delimiter_actions_follow_retained_hover_and_view_ownership() {
    let owner = Root::document();
    let other = Root::document();
    let measured = in_view(
        native_delimiter(true, 1.0, Extent::default(), puri::delim::Side::Open),
        owner.clone(),
    );
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    let rect = measured.extent.rect_at(Point::new(20.0, 30.0));
    let target = Hovered::Tree(crate::hover::Hover::Value(std::rc::Rc::from([])));
    let point = Point::new(rect.x1 + 1.0, rect.center().y);
    let placed = crate::display::widget::frame::place(
        measured,
        Placement::new(rect, bounds),
        &HoverInput {
            pointer: Some(point),
            prior: Some(&target),
            reach_px: 3.0,
            ..Default::default()
        },
    );
    assert!(matches!(
        placed.claim.clone().map(|(_, claim)| claim),
        Some(Claim::Extended(_))
    ));
    let placed = placed.bind(Default::default());
    for (root, hovered, picking, expected) in [
        (
            Some(owner.clone()),
            Some(target.clone()),
            false,
            Some("select"),
        ),
        (
            Some(owner.clone()),
            Some(target.clone()),
            true,
            Some("pick"),
        ),
        (Some(other), Some(target.clone()), false, None),
        (Some(owner), None, false, None),
    ] {
        let event = press_at(
            point,
            if picking {
                crate::modifiers::command()
            } else {
                ui_events::keyboard::Modifiers::empty()
            },
        );
        let mut world = crate::test_editor(gid::Document::default());
        if picking {
            world.model.selection = Some(crate::selection::pending_value(
                world.model.workspace.document_root(),
                vec![],
            ));
        }
        let handled = placed.handler.as_ref().unwrap().dispatch_pointer_down_with(
            &mut world,
            &event,
            &mut DispatchContext::new(root, hovered),
        );
        assert_eq!(handled, expected.is_some());
        match expected {
            Some("pick") => assert_eq!(world.model.doc.root, Some(gid::Value::record([]))),
            Some("select") => assert!(world.model.selection.is_some()),
            _ => {}
        }
    }
}

#[test]
fn clipped_native_delimiters_add_no_ink_hover_or_actions() {
    let measured = native_delimiter(true, 1.0, Extent::default(), puri::delim::Side::Close);
    let rect = measured.extent.rect_at(Point::new(20.0, 30.0));
    let placed = crate::display::widget::frame::place(
        measured,
        Placement::new(rect, Rect::ZERO),
        &Default::default(),
    );
    assert!(placed.claim.is_none());
    let placed = placed.bind(Default::default());
    assert!(placed.renders.is_empty());
    assert!(placed.handler.is_none());
}

fn move_at(x: f64, y: f64) -> PointerUpdate {
    PointerUpdate {
        pointer: mouse(),
        current: state_at(x, y),
        coalesced: Vec::new(),
        predicted: Vec::new(),
    }
}

fn scroll_at(x: f64, y: f64) -> PointerScrollEvent {
    PointerScrollEvent {
        pointer: mouse(),
        delta: ScrollDelta::LineDelta(0.0, 1.0),
        state: state_at(x, y),
    }
}

#[test]
fn target_actions_and_raw_handlers_share_visual_precedence() {
    let target = Hovered::Tree(crate::hover::Hover::Value(std::rc::Rc::from([])));
    let other = Hovered::Tree(crate::hover::Hover::Toggle(std::rc::Rc::from([])));
    let placement = Placement::root(Rect::new(0.0, 0.0, 20.0, 20.0));
    let mut frame = HoverOutput::default();
    let mut navigation = Default::default();
    let mut placed: HoverContext<'_, Vec<&'static str>, Hovered> = HoverContext::new(
        HoverInput {
            pointer: Some(Point::new(5.0, 5.0)),
            ..Default::default()
        },
        &mut frame,
        &mut navigation,
    );
    let mut p = Placed::new(&mut placed, placement);
    p.handler().on_pointer_down(|log, _| {
        log.push("raw below");
        true
    });
    p.activate(target.clone(), |log| {
        log.push("activation");
        true
    });
    p.activate(other, |_| panic!("wrong target"));
    p.handler().on_pointer_down(|log, _| {
        log.push("raw above declined");
        false
    });
    let mut placed = frame;
    let mut pointer = DispatchContext::new(None, Some(target));
    let mut log = Vec::new();
    assert!(placed.handler.take().unwrap().dispatch_pointer_down_with(
        &mut log,
        &press_at(
            Point::new(5.0, 5.0),
            ui_events::keyboard::Modifiers::empty()
        ),
        &mut pointer
    ));
    assert_eq!(log, ["raw above declined", "activation"]);
}

#[test]
fn direct_placement_preserves_handler_precedence() {
    let extent = Extent {
        width: 10.0,
        ascent: 5.0,
        descent: 5.0,
    };
    let lower = leaf(extent, |p: &mut Placed<'_, '_, Vec<&'static str>>, _| {
        p.handler().on_pointer_down(|log, _| {
            log.push("lower");
            true
        });
    });
    let upper = leaf(extent, |p: &mut Placed<'_, '_, Vec<&'static str>>, _| {
        p.handler().on_pointer_down(|log, _| {
            log.push("upper");
            false
        });
    });
    let placed = {
        let layout = measured::layers(vec![lower, upper]);
        let placement = uig::Placement::root(layout.extent.rect_at(Point::ZERO));
        crate::display::widget::frame::place(layout, placement, &Default::default())
    };
    let placed = placed.bind(Default::default());
    let handler = placed.handler.expect("registrations");
    let mut log = Vec::new();

    assert!(handler.dispatch_pointer_down(
        &mut log,
        &press_at(
            Point::new(5.0, 5.0),
            ui_events::keyboard::Modifiers::empty()
        )
    ));
    assert_eq!(log, ["upper", "lower"]);
}

#[test]
fn popover_content_is_out_of_flow_and_raised_over_the_frame() {
    let trigger = leaf(
        Extent {
            width: 10.0,
            ascent: 0.0,
            descent: 10.0,
        },
        |p: &mut Placed<'_, '_, ()>, placement| {
            p.fill(placement.rect, Color::BLACK, Affine::IDENTITY);
        },
    );
    let content = leaf(
        Extent {
            width: 20.0,
            ascent: 0.0,
            descent: 20.0,
        },
        |p: &mut Placed<'_, '_, ()>, placement| {
            p.fill(placement.rect, Color::WHITE, Affine::IDENTITY);
        },
    );
    let popup = floating(trigger, content, |placement, extent| {
        crate::display::widget::popover::position(placement, extent, 2.0)
    });

    assert_eq!(popup.extent.width, 10.0);
    assert_eq!(popup.extent.height(), 10.0);

    let placed = crate::display::widget::frame::place(
        popup,
        Placement::new(
            Rect::new(5.0, 5.0, 15.0, 15.0),
            Rect::new(0.0, 0.0, 100.0, 100.0),
        ),
        &Default::default(),
    );

    let mut canvas = TestCanvas(DrawList::new());
    puri::frame::render(placed.bind(no_ink()).renders, &mut canvas);
    assert!(matches!(
        &canvas.0.0[..],
        [
            DrawCmd::Fill {
                shape: Shape::Rect(trigger),
                ..
            },
            DrawCmd::Fill {
                shape: Shape::Rect(content),
                ..
            }
        ] if *trigger == Rect::new(5.0, 5.0, 15.0, 15.0)
            && *content == Rect::new(5.0, 17.0, 25.0, 37.0)
    ));
}

#[test]
fn clipped_leaves_keep_handlers_but_contribute_no_visible_work() {
    let target = Hovered::Tree(crate::hover::Hover::Value(std::rc::Rc::from([])));
    let child = leaf(
        Extent {
            width: 10.0,
            ascent: 5.0,
            descent: 5.0,
        },
        move |p: &mut Placed<'_, '_, ()>, placement| {
            p.claim(placement, target.clone());
            p.activate(target.clone(), |_| true);
            p.pick(target, |_| true);
            p.fill(placement.rect, Color::WHITE, Affine::IDENTITY);
            p.handler().on_pointer_move(|_, _| true);
        },
    );
    let placed = crate::display::widget::frame::place(
        child,
        Placement::new(
            Rect::new(20.0, 20.0, 30.0, 30.0),
            Rect::new(0.0, 0.0, 10.0, 10.0),
        ),
        &Default::default(),
    );

    assert!(placed.claim.is_none());
    let placed = placed.bind(Default::default());
    assert!(placed.renders.is_empty());
    assert!(placed.handler.is_some());
}

#[test]
fn scrolled_content_shifts_inside_the_viewport_clip() {
    let probe = leaf(
        Extent {
            width: 100.0,
            ascent: 0.0,
            descent: 300.0,
        },
        |p: &mut Placed<'_, '_, ()>, placement| {
            assert_eq!(placement.clip_rect, Rect::new(10.0, 20.0, 90.0, 70.0));
            p.fill(
                Rect::new(
                    placement.rect.x0,
                    placement.rect.y0,
                    placement.rect.x0 + 1.0,
                    placement.rect.y0 + 1.0,
                ),
                Color::WHITE,
                Affine::IDENTITY,
            );
        },
    );
    let placed = crate::display::widget::frame::place(
        scrolled(probe, Vec2::new(5.0, 40.0), 1.0, |_, event| {
            ScrollOutcome::pass(event)
        }),
        Placement::new(
            Rect::new(10.0, 20.0, 90.0, 70.0),
            Rect::new(0.0, 0.0, 100.0, 100.0),
        ),
        &Default::default(),
    );
    let mut canvas = TestCanvas(DrawList::new());
    puri::frame::render(placed.bind(no_ink()).renders, &mut canvas);
    let [
        DrawCmd::Clip {
            shape: Shape::Rect(clip),
            children,
            ..
        },
    ] = &canvas.0.0[..]
    else {
        panic!("expected one clip");
    };
    assert_eq!(*clip, Rect::new(10.0, 20.0, 90.0, 70.0));
    let [
        DrawCmd::Fill {
            shape: Shape::Rect(dot),
            ..
        },
    ] = &children[..]
    else {
        panic!("expected the probe inside the clip");
    };
    assert_eq!((dot.x0, dot.y0), (5.0, -20.0));
}

#[test]
fn scroll_viewport_bounds_starts_and_not_active_motion_or_release() {
    let child = leaf(
        Extent {
            width: 30.0,
            ascent: 0.0,
            descent: 30.0,
        },
        |p: &mut Placed<'_, '_, Vec<&'static str>>, _| {
            p.handler().on_pointer_down(|log, _| {
                log.push("down");
                true
            });
            p.handler().on_pointer_move(|log, _| {
                log.push("move");
                true
            });
            p.handler().on_pointer_up(|log, _| {
                log.push("up");
                true
            });
        },
    );
    let placed = crate::display::widget::frame::place(
        scrolled(
            child,
            Vec2::ZERO,
            1.0,
            |log: &mut Vec<&'static str>, event| {
                log.push("scroll");
                ScrollOutcome::consume(event)
            },
        ),
        Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)),
        &Default::default(),
    );
    let placed = placed.bind(Default::default());
    let handler = placed.handler.expect("registrations");
    let mut log = Vec::new();
    assert!(!handler.dispatch_pointer_down(
        &mut log,
        &press_at(
            Point::new(20.0, 5.0),
            ui_events::keyboard::Modifiers::empty()
        )
    ));
    assert!(
        !handler
            .dispatch_scroll(&mut log, &scroll_at(20.0, 5.0))
            .handled()
    );
    assert!(handler.dispatch_pointer_down(
        &mut log,
        &press_at(
            Point::new(5.0, 5.0),
            ui_events::keyboard::Modifiers::empty()
        )
    ));
    assert!(
        handler
            .dispatch_scroll(&mut log, &scroll_at(5.0, 5.0))
            .handled()
    );
    assert!(handler.dispatch_pointer_move(&mut log, &move_at(20.0, 5.0)));
    assert!(handler.dispatch_pointer_up(
        &mut log,
        &press_at(
            Point::new(20.0, 5.0),
            ui_events::keyboard::Modifiers::empty()
        )
    ));
    assert_eq!(log, ["down", "scroll", "move", "up"]);
}
#[test]
fn occlusion_blocks_clicks_in_its_clip_but_not_active_gestures() {
    let mut frame = HoverOutput::default();
    let mut navigation = Default::default();
    let mut placed: HoverContext<'_, Vec<&'static str>, Hovered> = HoverContext::new(
        HoverInput {
            pointer: Some(Point::new(5.0, 5.0)),
            ..Default::default()
        },
        &mut frame,
        &mut navigation,
    );
    let full = Placement::root(Rect::new(0.0, 0.0, 100.0, 100.0));
    let mut p = Placed::new(&mut placed, full);
    p.handler().on_pointer_down(|log, _| {
        log.push("covered");
        true
    });
    p.handler().on_pointer_move(|log, _| {
        log.push("move");
        true
    });
    p.handler().on_pointer_up(|log, _| {
        log.push("up");
        true
    });
    p.occlude(Placement::new(full.rect, Rect::new(10.0, 10.0, 20.0, 20.0)));
    let placed = frame;
    let placed = placed.bind(Default::default());
    let handler = placed.handler.unwrap();
    let mut log = Vec::new();
    for button in [PointerButton::Primary, PointerButton::Secondary] {
        let event = PointerButtonEvent {
            button: Some(button),
            ..press_at(
                Point::new(15.0, 15.0),
                ui_events::keyboard::Modifiers::empty(),
            )
        };
        assert!(handler.dispatch_pointer_down(&mut log, &event));
        assert!(log.is_empty());
    }
    assert!(handler.dispatch_pointer_down(
        &mut log,
        &press_at(
            Point::new(25.0, 15.0),
            ui_events::keyboard::Modifiers::empty()
        )
    ));
    assert!(handler.dispatch_pointer_move(&mut log, &move_at(150.0, 150.0)));
    assert!(handler.dispatch_pointer_up(
        &mut log,
        &press_at(
            Point::new(150.0, 150.0),
            ui_events::keyboard::Modifiers::empty()
        )
    ));
    assert_eq!(log, ["covered", "move", "up"]);
}

#[test]
fn a_floater_keeps_its_view_when_it_covers_another_view() {
    let owner = Root::document();
    let covered = Root::document();
    let target = Hovered::Tree(crate::hover::Hover::Entry(0));
    let extent = Extent {
        width: 10.0,
        ascent: 0.0,
        descent: 10.0,
    };
    let row = |label: &'static str| {
        let target = target.clone();
        leaf(
            extent,
            move |p: &mut Placed<'_, '_, Vec<&'static str>>, placement| {
                p.claim(placement, target.clone());
                p.activate(target.clone(), move |log| {
                    log.push(label);
                    true
                });
                p.pick(target, move |log| {
                    log.push(label);
                    true
                });
            },
        )
    };
    let popup_rect = Rect::new(50.0, 0.0, 60.0, 10.0);
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    let owner_view = in_view(
        floating(
            leaf(extent, |_: &mut Placed<'_, '_, Vec<&'static str>>, _| {}),
            row("popup"),
            move |_, _| Some(Placement::new(popup_rect, bounds)),
        ),
        owner.clone(),
    );
    let mut pass = HoverPass::new(&HoverInput {
        pointer: Some(popup_rect.center()),
        ..Default::default()
    });
    measured::place_into(
        owner_view,
        Placement::new(extent.rect_at(Point::ZERO), bounds),
        &mut pass,
    );
    measured::place_into(
        in_view(row("covered pane"), covered),
        Placement::new(popup_rect, bounds),
        &mut pass,
    );
    let placed = pass.finish();
    let point = popup_rect.center();
    let (root, Claim::Direct(hit)) = placed.claim.clone().unwrap() else {
        panic!("popup hover")
    };
    let placed = placed.bind(Default::default());
    assert!(root.as_ref() == Some(&owner));
    for modifiers in [
        ui_events::keyboard::Modifiers::empty(),
        crate::modifiers::command(),
    ] {
        let mut pointer = DispatchContext::new(root.clone(), Some(hit.clone()));
        let event = press_at(point, modifiers);
        let mut log = Vec::new();
        assert!(placed.handler.as_ref().unwrap().dispatch_pointer_down_with(
            &mut log,
            &event,
            &mut pointer
        ));
        assert_eq!(log, ["popup"]);
    }
}

#[test]
fn native_floaters_keep_their_view_and_raise_above_later_content() {
    use crate::display::widget;
    let owner = Root::document();
    let other = Root::pane(vec![]);
    let target = Hovered::Tree(crate::hover::Hover::Entry(0));
    let expected = target.clone();
    let rect = Rect::new(50.0, 0.0, 70.0, 20.0);
    let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
    let popup = widget::leaf(
        Extent {
            width: 20.0,
            ascent: 0.0,
            descent: 20.0,
        },
        move |output: &mut widget::HoverContext<'_, usize, _>, placement| {
            output.claim(crate::display::widget::frame::Probe::retaining(
                placement,
                target.clone(),
            ));
            output
                .handler()
                .on_pointer_down_with(move |count, _, hovered| {
                    hovered.hovered() == Some(&target) && {
                        *count += 1;
                        true
                    }
                });
        },
    );
    let popup = widget::navigation::landmark(
        popup,
        std::rc::Rc::from([gid::Step::Follow(gid::Resolution::Document)]),
        std::rc::Rc::new(|count, _| {
            *count += 10;
            true
        }),
        Default::default(),
        None,
        false,
    );
    let widget = container::floating(
        widget::leaf(Extent::default(), |_, _| {}),
        popup,
        move |_, _| Some(Placement::new(rect, bounds)),
    );
    let native = measured::leaf_into(widget.extent, move |placement, pass| {
        measured::place_into(widget, placement, pass)
    });
    let mut pass = HoverPass::new(&HoverInput {
        pointer: Some(rect.center()),
        ..Default::default()
    });
    measured::place_into(
        in_view(native, owner.clone()),
        Placement::root(bounds),
        &mut pass,
    );
    measured::place_into(
        in_view(
            leaf(Extent::default(), |output, placement| {
                output.occlude(placement)
            }),
            other.clone(),
        ),
        Placement::root(bounds),
        &mut pass,
    );
    let output = pass.finish();
    let (root, claim) = output.claim.clone().unwrap();
    let output = output.bind(Default::default());
    let [landmark] = output.landmarks.as_slice() else {
        panic!("native popup must contribute exactly one landmark");
    };
    assert_eq!(landmark.root, Some(owner.clone()));
    assert_eq!(landmark.rect, rect);
    assert_eq!(
        landmark.path.as_ref(),
        &[gid::Step::Follow(gid::Resolution::Document)]
    );
    let mut count = 0;
    assert!((landmark.select)(
        &mut count,
        Some(puri::handler::NavigationDirection::Left)
    ));
    assert_eq!(count, 10);
    assert_eq!(root, Some(owner.clone()));
    assert_eq!(claim, Claim::Direct(expected.clone()));
    let handler = output.handler.unwrap();
    let mut count = 0;
    assert!(handler.dispatch_pointer_down_with(
        &mut count,
        &press_at(
            Point::new(55.0, 5.0),
            ui_events::keyboard::Modifiers::empty()
        ),
        &mut DispatchContext::new(Some(owner), Some(expected))
    ));
    assert_eq!(count, 1);
}

#[test]
fn semantic_actions_obey_scroll_clips_even_when_the_target_matches() {
    let target = Hovered::Tree(crate::hover::Hover::Value(std::rc::Rc::from([])));
    let claimed = target.clone();
    let child = leaf(
        Extent {
            width: 30.0,
            ascent: 0.0,
            descent: 30.0,
        },
        move |p: &mut Placed<'_, '_, usize>, placement| {
            p.claim(placement, claimed.clone());
            p.activate(claimed, |count| {
                *count += 1;
                true
            });
        },
    );
    let placed = crate::display::widget::frame::place(
        scrolled(child, Vec2::ZERO, 1.0, |_, event| {
            ScrollOutcome::pass(event)
        }),
        Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)),
        &Default::default(),
    );
    let mut pointer = DispatchContext::new(None, Some(target));
    let placed = placed.bind(Default::default());
    let handler = placed.handler.unwrap();
    let mut count = 0;
    assert!(!handler.dispatch_pointer_down_with(
        &mut count,
        &press_at(
            Point::new(20.0, 5.0),
            ui_events::keyboard::Modifiers::empty()
        ),
        &mut pointer
    ));
    assert_eq!(count, 0);
    assert!(handler.dispatch_pointer_down_with(
        &mut count,
        &press_at(
            Point::new(5.0, 5.0),
            ui_events::keyboard::Modifiers::empty()
        ),
        &mut pointer
    ));
    assert_eq!(count, 1);
}
#[test]
fn source_picks_share_pointer_order_and_respect_occlusion() {
    let target = Hovered::Tree(crate::hover::Hover::Source(
        crate::display::widget::source::SourceTrace::Stored(std::rc::Rc::from([])),
    ));
    let placement = Placement::root(Rect::new(0.0, 0.0, 20.0, 20.0));
    for covered in [false, true] {
        let mut editor = crate::test_editor(gid::Document::default());
        let count = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut frame = HoverOutput::default();
        let mut navigation = Default::default();
        let mut placed: HoverContext<'_, crate::Editor, Hovered> = HoverContext::new(
            HoverInput {
                pointer: Some(Point::new(5.0, 5.0)),
                ..Default::default()
            },
            &mut frame,
            &mut navigation,
        );
        let mut p = Placed::new(&mut placed, placement);
        p.handler()
            .on_pointer_down(|_, _| panic!("covered raw handler"));
        p.occlude(placement);
        let claimed = target.clone();
        p.claim_dynamic(placement, move |_| Some(claimed.clone()));
        crate::display::widget::source_link::handlers(
            &mut p,
            placement,
            1.0,
            crate::modifiers::native(),
        );
        if covered {
            p.occlude(placement);
        }
        let mut placed = frame;
        let hovered = match placed.claim.take().map(|(_, claim)| claim) {
            Some(Claim::Direct(target)) => target,
            Some(Claim::Occludes) => Hovered::Blocked,
            _ => panic!("hit"),
        };
        let mut pointer = DispatchContext::new(None, Some(hovered));
        pointer.landmarks = std::rc::Rc::from([crate::display::widget::navigation::Landmark {
            root: Some(editor.model.workspace.document_root().clone()),
            scope: Default::default(),
            path: std::rc::Rc::from([]),
            rect: placement.rect,
            select: std::rc::Rc::new({
                let count = count.clone();
                move |_, _| {
                    count.set(count.get() + 1);
                    true
                }
            }),
            secondary: None,
            selected: false,
            tab_stop: false,
        }]);
        let event = press_at(Point::new(5.0, 5.0), crate::modifiers::command());
        assert!(placed.handler.take().unwrap().dispatch_pointer_down_with(
            &mut editor,
            &event,
            &mut pointer
        ));
        assert_eq!(count.get(), usize::from(!covered));
    }
}
