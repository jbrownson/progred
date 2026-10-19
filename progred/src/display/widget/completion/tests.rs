use super::*;
use crate::display::Face;
use crate::display::test_support::mouse;
use crate::display::test_support::{ResolveForDispatch, painted};
use crate::display::widget::frame::{Attribution, DispatchContext, HoverInput, HoverOutput, place};
use crate::display::widget::style::{Theme, editor, primary_highlight};
use crate::frame::Hovered;
use crate::test_support::key_down;
use gid::Value;
use kurbo::Rect;
use peniko::Brush;
use puri::draw::{DrawCmd, DrawList, Shape};
use puri::hover::Claim;
use puri::text::TextCache;
use ui_events::ScrollDelta;
use ui_events::keyboard::Modifiers;
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerScrollEvent, PointerState,
    PointerUpdate,
};

/// What measuring a card's text needs, as a frame lends it.
struct Fonts {
    fonts: parley::FontContext,
    layouts: parley::LayoutContext<Brush>,
    cache: TextCache,
}

impl Fonts {
    fn new() -> Self {
        Self {
            fonts: crate::fonts::bundled_font_context(),
            layouts: parley::LayoutContext::new(),
            cache: TextCache::default(),
        }
    }

    fn text(&mut self, scale: f64) -> TextCtx<'_> {
        TextCtx {
            fonts: &mut self.fonts,
            layouts: &mut self.layouts,
            cache: &mut self.cache,
            scale: scale as f32,
        }
    }
}

/// What a placed card paints with nothing hovered.
fn paint<C: 'static>(placed: HoverOutput<C, Hovered>) -> DrawList {
    let mut list = DrawList::new();
    puri::frame::render(placed.bind(Attribution::default()).renders, &mut list);
    list
}

#[test]
fn a_completion_without_an_edit_still_consumes_its_activation() {
    let entries = [Entry {
        display: "no edit".into(),
        detail: None,
        matches: vec![],
        face: Face::Label,
        source: None,
        activate: Rc::new(|attempts: &mut usize| {
            *attempts += 1;
        }),
    }];
    let mut fonts = Fonts::new();
    let placed = {
        let layout = card::<usize, Hovered>(
            &mut fonts.text(1.0),
            &editor(Theme::Light.palette(), 1.0),
            &entries,
            State::default(),
            |_| Some(0.0),
            |_, _| {},
        );
        let placement = Placement::root(layout.extent.rect_at(Point::ZERO));
        place(layout, placement, &Default::default())
    };
    let mut attempts = 0;
    assert!(placed.resolve_for_dispatch().dispatch_key(
        &mut attempts,
        &key_down(NamedKey::Enter, Modifiers::empty())
    ));
    assert_eq!(attempts, 1);
}

#[test]
fn completion_popup_meets_the_painted_field_border_above_and_below() {
    for scale in [1.0, 1.5, 2.0] {
        let mut fonts = Fonts::new();
        let styles = editor(Theme::Light.palette(), scale);
        let mut tcx = fonts.text(scale);
        let entries = [Entry {
            display: "entry".into(),
            detail: None,
            matches: Vec::new(),
            face: Face::Name,
            source: None,
            activate: Rc::new(|_: &mut ()| {}),
        }];
        let bounds = Rect::new(0.0, 0.0, 640.0 * scale, 480.0 * scale);
        for above in [false, true] {
            let y = if above { 450.0 } else { 30.0 } * scale;
            let field = Rect::new(50.0 * scale, y, 150.0 * scale, y + 20.0 * scale);
            let layout = card::<(), Hovered>(
                &mut tcx,
                &styles,
                &entries,
                State::default(),
                |_| Some(0.0),
                |_, _| {},
            );
            let floated = placement(Placement::new(field, bounds), layout.extent, scale).unwrap();
            let mut drawn = paint(place(layout, floated, &Default::default()));
            primary_highlight(
                styles.palette,
                scale,
                &mut drawn,
                text_frame::outline(scale, field),
            );
            let edge = |color| {
                painted(&drawn.0)
                    .find_map(|command| match command {
                        DrawCmd::Stroke {
                            shape: Shape::RoundedRect(rect),
                            style,
                            brush,
                            ..
                        } if *brush == Brush::from(color) => {
                            Some(rect.rect().inflate(style.width / 2.0, style.width / 2.0))
                        }
                        _ => None,
                    })
                    .unwrap()
            };
            let popup = edge(styles.palette.border);
            let field = edge(styles.palette.accent);
            let separation = if above {
                field.y0 - popup.y1
            } else {
                popup.y0 - field.y1
            };
            assert!(separation.abs() < 1e-6);
            assert!((field.x0 - popup.x0).abs() < 1e-6);
            assert!(popup.x0 >= bounds.x0 && popup.x1 <= bounds.x1);
            assert!(popup.y0 >= bounds.y0 && popup.y1 <= bounds.y1);
        }
    }
}

#[test]
fn completion_details_share_the_cards_right_edge() {
    for scale in [1.0, 2.0] {
        let mut fonts = Fonts::new();
        let styles = editor(Theme::Light.palette(), scale);
        let mut tcx = fonts.text(scale);
        let entries = [
            ("x", "a long library name"),
            ("a much longer label", "short"),
        ]
        .map(|(display, detail)| Entry {
            display: display.into(),
            detail: Some(detail.into()),
            matches: Vec::new(),
            face: Face::Name,
            source: None,
            activate: Rc::new(|_: &mut ()| {}),
        });
        let detail_widths = entries.each_ref().map(|entry| {
            puri::text::text(&mut tcx, entry.detail.as_deref().unwrap(), &styles.detail)
                .metrics()
                .width
        });
        let layout = card::<(), Hovered>(
            &mut tcx,
            &styles,
            &entries,
            State::default(),
            |_| Some(0.0),
            |_, _| {},
        );
        let origin = Point::new(37.0, 59.0);
        let right = origin.x + layout.extent.width - (4.0 + 8.0) * scale;
        let placement = Placement::root(layout.extent.rect_at(origin));
        let drawn = paint(place(layout, placement, &Default::default()));
        let details = painted(&drawn.0)
            .filter_map(|command| match command {
                DrawCmd::GlyphRun(run) if run.brush == styles.detail.brush => Some(run),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(details.len(), detail_widths.len());
        for (run, width) in details.into_iter().zip(detail_widths) {
            assert!(((run.transform * Point::ZERO).x + width - right).abs() < 1e-6);
        }
    }
}

#[test]
fn completion_rows_claim_their_entries_and_the_card_occludes() {
    let entries = vec![
        Entry {
            display: "\"x\"".to_string(),
            detail: None,
            matches: Vec::new(),
            face: Face::String,
            source: None,
            activate: Rc::new(|_: &mut ()| {}),
        },
        Entry {
            display: "new list".to_string(),
            detail: None,
            matches: Vec::new(),
            face: Face::Dim,
            source: None,
            activate: Rc::new(|_: &mut ()| {}),
        },
    ];
    let place_card = |pointer| {
        let mut fonts = Fonts::new();
        let layout = card::<(), Hovered>(
            &mut fonts.text(1.0),
            &editor(Theme::Light.palette(), 1.0),
            &entries,
            State::default(),
            |_| Some(0.0),
            |_, _| {},
        );
        let extent = layout.extent;
        let placement = Placement::root(layout.extent.rect_at(Point::ZERO));
        let placed = place(
            layout,
            placement,
            &HoverInput {
                pointer: Some(pointer),
                ..Default::default()
            },
        );
        (placed.claim.map(|(_, claim)| claim), extent)
    };
    // The card's own padding claims-and-clears: an overlay's
    // pointer never falls through to what sits beneath it.
    let (padding, extent) = place_card(Point::new(1.0, 1.0));
    assert_eq!(padding, Some(Claim::Occludes));
    // Scanning down the card crosses both rows, each claiming its
    // index — an address into the frame's exact visible offers.
    let winners: Vec<Hover> = (0..extent.height() as usize)
        .filter_map(
            |y| match place_card(Point::new(extent.width / 2.0, y as f64 + 0.5)).0 {
                Some(Claim::Direct(Hovered::Tree(hover))) => Some(hover),
                _ => None,
            },
        )
        .collect();
    assert!(winners.contains(&Hover::Entry(0)));
    assert!(winners.contains(&Hover::Entry(1)));
}

#[test]
fn completion_viewport_scrolls_without_losing_keyboard_reveal() {
    let entries: Vec<_> = (0..20)
        .map(|index| Entry {
            display: format!("entry {index}"),
            detail: None,
            matches: Vec::new(),
            face: Face::Dim,
            source: None,
            activate: Rc::new(|_| {}),
        })
        .collect();
    let styles = editor(Theme::Light.palette(), 1.0);
    let mut fonts = Fonts::new();
    let mut tcx = fonts.text(1.0);
    let mut frame = |(scroll, choice), pointer| {
        let layout = card::<(f64, usize), Hovered>(
            &mut tcx,
            &styles,
            &entries,
            State { choice, scroll },
            |state| Some(state.0),
            |state, view| *state = (view.scroll, view.choice),
        );
        let placement = Placement::root(layout.extent.rect_at(Point::ZERO));
        place(
            layout,
            placement,
            &HoverInput {
                pointer,
                ..Default::default()
            },
        )
    };
    let mut state = (0.0, 0);
    assert_eq!(
        frame(state, Some(Point::new(10.0, 10.0)))
            .claim
            .map(|(_, claim)| claim),
        Some(Claim::Direct(Hovered::Tree(Hover::Entry(0)))),
    );
    let pointer = mouse();
    let mut pointer_state = PointerState::default();
    pointer_state.position.x = 10.0;
    pointer_state.position.y = 10.0;
    let scroll = PointerScrollEvent {
        pointer,
        state: pointer_state,
        delta: ScrollDelta::LineDelta(0.0, -5.0),
    };
    let initial = frame(state, None);
    assert!(
        initial
            .scroll_probes
            .iter()
            .any(|probe| probe.captures(&scroll))
    );
    let mut upward = scroll.clone();
    upward.delta = ScrollDelta::LineDelta(0.0, 5.0);
    assert!(
        !initial
            .scroll_probes
            .iter()
            .any(|probe| probe.captures(&upward))
    );
    assert!(
        initial
            .resolve_for_dispatch()
            .dispatch_scroll(&mut state, &scroll)
            .handled()
    );
    assert_eq!(state, (200.0, 0));
    let mut batched = (0.0, 0);
    let events = [scroll.clone(), scroll.clone()].map(|mut event| {
        event.delta = ScrollDelta::LineDelta(0.0, -2.5);
        event
    });
    assert!(
        frame(batched, None)
            .resolve_for_dispatch()
            .dispatch(
                &mut batched,
                puri::handler::Event::Scroll(std::borrow::Cow::Borrowed(&events)),
                &mut Default::default(),
            )
            .handled()
    );
    assert_eq!(batched, (200.0, 0));
    assert!((4..160).any(|y| matches!(
        frame(state, Some(Point::new(10.0,y as f64))).claim.map(|(_, claim)| claim),
        Some(Claim::Direct(Hovered::Tree(Hover::Entry(index)))) if index > 0
    )));
    assert!((0..200).all(|y| {
        frame(state, Some(Point::new(10.0, y as f64)))
            .claim
            .map(|(_, claim)| claim)
            != Some(Claim::Direct(Hovered::Tree(Hover::Entry(0))))
    }));
    for key in [NamedKey::ArrowDown, NamedKey::ArrowUp] {
        assert!(
            frame(state, None)
                .resolve_for_dispatch()
                .dispatch_key(&mut state, &key_down(key, Modifiers::empty()))
        );
    }
    assert_eq!(state, (0.0, 0));
    for _ in 0..12 {
        frame(state, None).resolve_for_dispatch().dispatch_key(
            &mut state,
            &key_down(NamedKey::ArrowDown, Modifiers::empty()),
        );
    }
    let offset = state.0;
    assert!(offset > 0.0);
    frame(state, None)
        .resolve_for_dispatch()
        .dispatch_key(&mut state, &key_down(NamedKey::ArrowUp, Modifiers::empty()));
    assert_eq!(state, (offset, 11));
    for _ in 11..entries.len() {
        frame(state, None).resolve_for_dispatch().dispatch_key(
            &mut state,
            &key_down(NamedKey::ArrowDown, Modifiers::empty()),
        );
    }
    // Down stops at the last row.
    assert_eq!(state.1, entries.len() - 1);
    // Tab is the editor's, for moving on to the next empty slot: the card
    // leaves it alone.
    let before_tab = state;
    assert!(
        !frame(state, None)
            .resolve_for_dispatch()
            .dispatch_key(&mut state, &key_down(NamedKey::Tab, Modifiers::empty()))
    );
    assert_eq!(state, before_tab);
}

#[test]
fn a_card_placed_shorter_scrolls_what_it_cannot_show() {
    let entries: Vec<_> = (0..20)
        .map(|index| Entry {
            display: format!("entry {index}"),
            detail: None,
            matches: Vec::new(),
            face: Face::Dim,
            source: None,
            activate: Rc::new(|_| {}),
        })
        .collect();
    let styles = editor(Theme::Light.palette(), 1.0);
    let mut fonts = Fonts::new();
    let mut tcx = fonts.text(1.0);
    let height = 60.0;
    let mut hovered = |(scroll, choice), pointer: Option<Point>| {
        let layout = card::<(f64, usize), Hovered>(
            &mut tcx,
            &styles,
            &entries,
            State { choice, scroll },
            |state| Some(state.0),
            |state, view| *state = (view.scroll, view.choice),
        );
        let natural = layout.extent.rect_at(Point::ZERO);
        assert!(natural.height() > 2.0 * height);
        let shortened = Rect::from_origin_size(Point::ZERO, (natural.width(), height));
        place(
            layout,
            Placement::root(shortened),
            &HoverInput {
                pointer,
                ..Default::default()
            },
        )
    };
    let entry = |output: HoverOutput<(f64, usize), Hovered>| match output.claim {
        Some((_, Claim::Direct(Hovered::Tree(Hover::Entry(index))))) => Some(index),
        _ => None,
    };
    let mut state = (0.0, 0);
    assert_eq!(entry(hovered(state, Some(Point::new(10.0, 10.0)))), Some(0));
    // The card ends where it was placed, though it measured taller.
    assert!(
        hovered(state, Some(Point::new(10.0, height + 20.0)))
            .claim
            .is_none()
    );
    let down = key_down(NamedKey::ArrowDown, Modifiers::empty());
    for _ in 0..3 {
        hovered(state, None)
            .resolve_for_dispatch()
            .dispatch_key(&mut state, &down);
    }
    // A fourth row is past what a 60-point card shows, so choosing it
    // scrolls, which eight rows wouldn't, and it can be pointed at.
    assert_eq!(state.1, 3);
    assert!(state.0 > 0.0);
    let mut shown = |y| entry(hovered(state, Some(Point::new(10.0, y as f64))));
    assert!((0..height as usize).any(|y| shown(y) == Some(3)));
}

#[test]
fn completion_has_one_choice_shared_by_mouse_and_keyboard_navigation() {
    #[derive(Default)]
    struct World {
        view: (f64, usize),
        committed: Option<usize>,
    }
    let entries = (0..3)
        .map(|index| Entry {
            display: format!("entry {index}"),
            detail: None,
            matches: Vec::new(),
            face: Face::Name,
            source: None,
            activate: Rc::new(move |world: &mut World| world.committed = Some(index)),
        })
        .collect::<Vec<_>>();
    let styles = editor(Theme::Light.palette(), 1.0);
    let mut fonts = Fonts::new();
    let mut tcx = fonts.text(1.0);
    let mut frame = |world: &World, clip: Option<Rect>, pointer| {
        let layout = card::<World, Hovered>(
            &mut tcx,
            &styles,
            &entries,
            State {
                choice: world.view.1,
                scroll: world.view.0,
            },
            |world| Some(world.view.0),
            |world, view| world.view = (view.scroll, view.choice),
        );
        let rect = layout.extent.rect_at(Point::ZERO);
        place(
            layout,
            Placement::new(rect, clip.unwrap_or(rect)),
            &HoverInput {
                pointer,
                ..Default::default()
            },
        )
    };
    let mut row_point = |world: &World, hover: Hover| {
        (0..160)
            .map(|y| Point::new(10.0, y as f64))
            .find(|point| {
                frame(world, None, Some(*point))
                    .claim
                    .map(|(_, claim)| claim)
                    == Some(Claim::Direct(Hovered::Tree(hover.clone())))
            })
            .unwrap()
    };
    let highlight = |placed: HoverOutput<World, Hovered>, hovered, selected_point| {
        let mut drawing = DrawList::new();
        let hovered = Hovered::Tree(hovered);
        puri::frame::render(
            placed
                .bind(Attribution {
                    hovered: Some(hovered),
                    ..Default::default()
                })
                .renders,
            &mut drawing,
        );
        fn highlights(commands: &[DrawCmd]) -> Vec<Rect> {
            let washes = [0.14, 0.08].map(|alpha| Theme::Light.palette().accent.with_alpha(alpha));
            painted(commands)
                .filter_map(|command| match command {
                    DrawCmd::Fill {
                        shape: Shape::RoundedRect(rect),
                        brush: Brush::Solid(color),
                        ..
                    } if washes.contains(color) => Some(rect.rect()),
                    _ => None,
                })
                .collect()
        }
        let rects = highlights(&drawing.0);
        assert_eq!(rects.len(), 1);
        assert!(rects[0].contains(selected_point));
    };
    let move_to = |point: Point, pointer_type| {
        let mut current = PointerState::default();
        current.position.x = point.x;
        current.position.y = point.y;
        PointerUpdate {
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type,
            },
            current,
            coalesced: Vec::new(),
            predicted: Vec::new(),
        }
    };
    let mut world = World::default();
    let points = std::array::from_fn::<_, 3, _>(|index| row_point(&world, Hover::Entry(index)));
    highlight(frame(&world, None, None), Hover::Entry(2), points[0]);

    assert!(
        frame(&world, None, None)
            .resolve_for_dispatch()
            .dispatch_pointer_move(&mut world, &move_to(points[2], PointerType::Mouse))
    );
    assert_eq!(world.view.1, 2);
    highlight(frame(&world, None, None), Hover::Entry(2), points[2]);
    assert!(
        frame(&world, None, None)
            .resolve_for_dispatch()
            .dispatch_key(&mut world, &key_down(NamedKey::ArrowUp, Modifiers::empty()))
    );
    assert_eq!(world.view.1, 1);
    highlight(frame(&world, None, None), Hover::Entry(2), points[1]);
    frame(&world, None, None)
        .resolve_for_dispatch()
        .dispatch_key(&mut world, &key_down(NamedKey::Enter, Modifiers::empty()));
    assert_eq!(world.committed, Some(1));

    frame(&world, None, None)
        .resolve_for_dispatch()
        .dispatch_pointer_move(&mut world, &move_to(points[0], PointerType::Mouse));
    assert_eq!(world.view.1, 0);
    frame(&world, None, None)
        .resolve_for_dispatch()
        .dispatch_key(&mut world, &key_down(NamedKey::Enter, Modifiers::empty()));
    assert_eq!(world.committed, Some(0));

    let mut dragging = move_to(points[2], PointerType::Mouse);
    dragging.current.buttons.insert(PointerButton::Primary);
    for event in [
        move_to(points[2], PointerType::Touch),
        dragging,
        move_to(Point::new(-10.0, -10.0), PointerType::Mouse),
    ] {
        assert!(
            !frame(&world, None, None)
                .resolve_for_dispatch()
                .dispatch_pointer_move(&mut world, &event)
        );
        assert_eq!(world.view.1, 0);
    }
    assert!(
        !frame(&world, Some(Rect::new(0.0, 0.0, 200.0, points[1].y)), None)
            .resolve_for_dispatch()
            .dispatch_pointer_move(&mut world, &move_to(points[2], PointerType::Mouse))
    );
    assert_eq!(world.view.1, 0);
}

#[test]
fn completion_rows_activate_their_own_action_by_keyboard_or_pointer() {
    #[derive(Default)]
    struct World {
        view: (f64, usize),
        committed: Option<Value>,
    }

    let entries = [Entry {
        display: "new list".into(),
        detail: None,
        matches: Vec::new(),
        face: Face::Dim,
        source: None,
        activate: Rc::new(|world: &mut World| {
            world.committed = Some(Value::list([]));
        }),
    }];
    let styles = editor(Theme::Light.palette(), 1.0);
    let mut fonts = Fonts::new();
    let mut tcx = fonts.text(1.0);
    let mut frame = |world: &World, entries: &[Entry<World>], pointer| {
        let layout = card::<World, Hovered>(
            &mut tcx,
            &styles,
            entries,
            State {
                choice: world.view.1,
                scroll: world.view.0,
            },
            |world| Some(world.view.0),
            |world, view| world.view = (view.scroll, view.choice),
        );
        let placement = Placement::root(layout.extent.rect_at(Point::ZERO));
        place(
            layout,
            placement,
            &HoverInput {
                pointer,
                ..Default::default()
            },
        )
    };
    let mut world = World::default();
    assert!(
        frame(&world, &entries, None)
            .resolve_for_dispatch()
            .dispatch_key(&mut world, &key_down(NamedKey::Enter, Modifiers::empty()))
    );
    assert_eq!(world.committed.take(), Some(Value::list([])));
    // Down stays on the only row.
    frame(&world, &entries, None)
        .resolve_for_dispatch()
        .dispatch_key(
            &mut world,
            &key_down(NamedKey::ArrowDown, Modifiers::empty()),
        );
    assert_eq!(world.view, (0.0, 0));
    let two = [
        entries[0].clone(),
        Entry {
            display: "new record".into(),
            activate: Rc::new(|world: &mut World| {
                world.committed = Some(Value::record([]));
            }),
            ..entries[0].clone()
        },
    ];
    frame(&world, &two, None)
        .resolve_for_dispatch()
        .dispatch_key(
            &mut world,
            &key_down(NamedKey::ArrowDown, Modifiers::empty()),
        );
    assert_eq!(world.view, (0.0, 1));
    assert!(
        frame(&world, &two, None)
            .resolve_for_dispatch()
            .dispatch_key(&mut world, &key_down(NamedKey::Enter, Modifiers::empty()))
    );
    assert_eq!(world.committed.take(), Some(Value::record([])));
    // A shorter list clamps the choice to its last row.
    assert!(
        frame(&world, &entries, None)
            .resolve_for_dispatch()
            .dispatch_key(&mut world, &key_down(NamedKey::Enter, Modifiers::empty()))
    );
    assert_eq!(world.committed.take(), Some(Value::list([])));

    // A click makes the row under the pointer, whichever is chosen.
    world.view = (0.0, 0);
    let placed = frame(&world, &two, None);
    let target = Hovered::Tree(Hover::Entry(1));
    let point = (0..100)
        .map(|y| Point::new(10.0, y as f64))
        .find(|point| {
            frame(&world, &two, Some(*point))
                .claim
                .map(|(_, claim)| claim)
                == Some(Claim::Direct(target.clone()))
        })
        .expect("the second row is visible");
    let mut pointer_state = PointerState::default();
    pointer_state.position.x = point.x;
    pointer_state.position.y = point.y;
    let event = PointerButtonEvent {
        button: Some(PointerButton::Primary),
        pointer: mouse(),
        state: pointer_state,
    };
    assert!(placed.resolve_for_dispatch().dispatch_pointer_down_with(
        &mut world,
        &event,
        &mut DispatchContext::new(None, Some(target)),
    ));
    assert_eq!(world.committed.take(), Some(Value::record([])));

    // With nothing offered, Enter has nothing to make.
    world.view = (0.0, 0);
    assert!(
        !frame(&world, &[], None)
            .resolve_for_dispatch()
            .dispatch_key(&mut world, &key_down(NamedKey::Enter, Modifiers::empty()))
    );
}

#[test]
fn completion_activation_precedes_the_real_editor_it_covers() {
    use crate::libraries::text;
    use gid::Step;
    // An empty record above a long line of text: typing over the record
    // opens a picker whose card covers the line.
    let root = Value::list([Value::record([]), text::value(&"long ".repeat(40))]);
    let [hole, line] = [0, 1].map(|index| {
        vec![Step::Element(
            crate::test_support::positions(&root)[index].clone(),
        )]
    });
    let mut driver =
        crate::driver::Driver::new(crate::editor_of(root), kurbo::Size::new(600.0, 400.0));
    driver.select(&hole);
    driver.type_text("\"t");
    assert_eq!(driver.picker_entries().unwrap()[0], "\"t\"");
    let covered = driver.rect(&line);
    let entry = Hovered::Tree(crate::hover::Hover::Entry(0));
    let point = (covered.y0.ceil() as usize..covered.y1 as usize)
        .flat_map(|y| {
            (covered.x0.ceil() as usize..covered.x1 as usize)
                .map(move |x| kurbo::Point::new(x as f64, y as f64))
        })
        .find(|point| driver.hover_at(*point).as_ref() == Some(&entry))
        .expect("the card's first entry covers the line");
    let beneath = driver.value(&line);
    driver.click_at(point);
    assert_eq!(
        driver.value(&hole),
        text::value("t"),
        "the entry took the click"
    );
    assert_eq!(driver.selection(), hole);
    assert_eq!(driver.value(&line), beneath);
}
