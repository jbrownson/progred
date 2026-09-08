use super::*;
use crate::libraries::f64 as f64_convention;

#[test]
fn sample_text_line_click_mounts_its_own_editor() {
    let (doc, _) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/sample.gid"
    )))
    .expect("the sample parses");
    let stack = crate::stack::load();
    let styles = crate::styles::editor(1.0);
    let mut fonts = parley::FontContext::new();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let mut tcx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let node = project(
        ProjectDescription {
            view: &crate::test_root(),
            completions: None,
            sources: Sources {
                doc: &doc,
                libraries: &stack.libraries,
            },
            root: doc.root.as_ref(),
            root_path: &[],
            selection: None,
            source_selection: None,
            annotations: &Annotations::default(),
            raw: false,
            styles: &styles,
            width: 852.0,

            projection: Some(&stack.projection),
        },
        &mut tcx,
    );
    let path = vec![
        Step::Key(sample_vocabulary::STYLE),
        Step::Follow(gid::Resolution::Document),
        Step::Key(sample_vocabulary::COLOR),
    ];
    let rect = node.extent.rect_at(Point::new(24.0, 24.0));
    let mut placed =
        crate::display::widget::frame::place(node, Placement::root(rect), &Default::default());
    let point = placed
        .descends
        .iter()
        .find(|descend| descend.path.as_ref() == &path)
        .expect("color descend")
        .rect
        .center();
    let mut state = ui_events::pointer::PointerState::default();
    state.position.x = point.x;
    state.position.y = point.y;
    let event = ui_events::pointer::PointerButtonEvent {
        button: Some(PointerButton::Primary),
        pointer: ui_events::pointer::PointerInfo {
            pointer_id: Some(ui_events::pointer::PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: ui_events::pointer::PointerType::Mouse,
        },
        state,
    };
    let mut world = crate::test_editor(doc.clone());
    assert!(
        placed
            .resolve_for_dispatch()
            .dispatch_pointer_down(&mut world, &event)
    );
    assert_eq!(
        world
            .model
            .selection
            .as_ref()
            .map(|selection| selection.path()),
        Some(path.as_slice())
    );

    // The next frame's focused editor still owns pointer-down: a
    // double click selects its word, and a later single click can
    // collapse that selection to a new caret. This is distinct from
    // the first click above, which mounts the editor.
    let mut frame_fonts = parley::FontContext::new();
    let mut frame_layouts = parley::LayoutContext::new();
    let mut frame_cache = puri::text::TextCache::default();
    let mut frame_tcx = TextCtx {
        fonts: &mut frame_fonts,
        layouts: &mut frame_layouts,
        scale: 1.0,
        cache: &mut frame_cache,
    };
    let active = project(
        ProjectDescription {
            view: &crate::test_root(),
            completions: None,
            sources: Sources {
                doc: &world.model.doc,
                libraries: &world.stack.libraries,
            },
            root: world.model.doc.root.as_ref(),
            root_path: &[],
            selection: world.model.selection.as_ref(),
            source_selection: world.model.selection.as_ref(),
            annotations: &Annotations::default(),
            raw: false,
            styles: &styles,
            width: 852.0,

            projection: Some(&stack.projection),
        },
        &mut frame_tcx,
    );
    let mut active =
        crate::display::widget::frame::place(active, Placement::root(rect), &Default::default());
    let line = active
        .descends
        .iter()
        .find(|descend| descend.path.as_ref() == &path)
        .expect("active color descend")
        .rect;
    let mut double = event.clone();
    double.state.position.x = line.center().x;
    double.state.position.y = line.center().y;
    double.state.count = 2;
    let handler = active.resolve_for_dispatch();
    assert!(handler.dispatch_pointer_down(&mut world, &double));
    let selection = world.model.selection.as_ref().unwrap().edit().unwrap();
    let (anchor, focus) = selection.selection_offsets();
    assert_ne!(anchor, focus);

    let mut single = double;
    single.state.position.x = line.x0 + 1.0;
    single.state.count = 1;
    assert!(handler.dispatch_pointer_down(&mut world, &single));
    let selection = world.model.selection.as_ref().unwrap().edit().unwrap();
    let (anchor, focus) = selection.selection_offsets();
    assert_eq!(anchor, focus);
}

fn target() -> Hovered {
    crate::libraries::test_widgets::hover(vec![])
}

fn press(x: f64, pick: bool) -> PointerButtonEvent {
    PointerButtonEvent {
        button: Some(PointerButton::Primary),
        pointer: PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        },
        state: PointerState {
            position: (x, 5.0).into(),
            modifiers: if pick {
                Modifiers::META | Modifiers::CONTROL
            } else {
                Modifiers::empty()
            },
            ..Default::default()
        },
    }
}

fn gesture_place(
    layout: crate::display::Layout<crate::Editor, Hovered>,
    readonly: bool,
) -> crate::display::widget::HoverCallback<crate::Editor, Hovered> {
    let crate::display::recording::Recorded::Before { before, .. } =
        crate::display::recording::record(&layout)
    else {
        panic!("widget wrapper");
    };
    crate::display::test_support::with_context(
        &crate::display::test_support::NoProject,
        |context| {
            let mut cx = context.inputs.clone();
            cx.source = if readonly {
                Source::Transient { owner: &[] }
            } else {
                Source::Stored
            };
            before(&mut crate::display::widget::Context {
                inputs: &cx,
                project: context.project,
                path: context.path,
                value: context.value,
                text: &mut *context.text,
            })
        },
    )
}

fn drag_frame(
    place: crate::display::widget::HoverCallback<crate::Editor, Hovered>,
    covered: bool,
) -> crate::placed::HoverOutput<crate::Editor> {
    let extent = Extent {
        width: 20.0,
        ascent: 0.0,
        descent: 20.0,
    };
    let node =
        crate::display::widget::before_place(leaf(extent, |_, _| {}), move |placement, output| {
            place(output, placement)
        });
    let placement = Placement::new(
        Rect::new(0.0, 0.0, 20.0, 20.0),
        Rect::new(0.0, 0.0, 10.0, 20.0),
    );
    let placed = crate::display::widget::frame::place(
        placed::in_view(node, crate::test_root()),
        placement,
        &Default::default(),
    );
    if covered {
        measured::Output::over(
            placed,
            crate::display::widget::frame::place(
                leaf(extent, |p, placement| p.occlude(placement)),
                placement,
                &Default::default(),
            ),
        )
    } else {
        placed
    }
}

#[test]
fn state_drag_press_composes_selection_and_start_in_pointer_order() {
    for (accepts, covered) in [(true, false), (false, false), (true, true)] {
        let log = Rc::new(std::cell::RefCell::new(Vec::new()));
        let select_log = log.clone();
        let start_log = log.clone();
        let mut frame = drag_frame(
            gesture_place(
                crate::display::on_state_drag(
                    crate::display::row(0.0, []),
                    target(),
                    Rc::new(move |world: &mut crate::Editor| {
                        select_log.borrow_mut().push("select");
                        if accepts {
                            crate::editing::select(world, &crate::test_root(), &[]);
                        }
                        accepts
                    }),
                    Rc::new(move || {
                        start_log.borrow_mut().push("start");
                        Box::new(|_, _| Value::record([]))
                    }),
                ),
                false,
            ),
            covered,
        );
        let mut world = crate::test_editor(Document {
            root: None,
            cells: Cells::new(),
        });
        let handled = frame.resolve_for_dispatch().dispatch_pointer_down_with(
            &mut world,
            &press(5.0, false),
            &mut placed::DispatchContext::new(Some(crate::test_root()), Some(target())),
        );
        assert_eq!(handled, covered || accepts);
        assert_eq!(world.gesture.is_some(), accepts && !covered);
        assert_eq!(world.model.selection.is_some(), accepts && !covered);
        assert_eq!(
            *log.borrow(),
            if covered {
                vec![]
            } else if accepts {
                vec!["select", "start"]
            } else {
                vec!["select"]
            }
        );
    }
}

#[test]
fn state_drag_starts_only_at_a_visible_primary_contact_in_its_own_view() {
    for (x, button, pointer_type, owns_view, expected) in [
        (
            5.0,
            Some(PointerButton::Primary),
            PointerType::Mouse,
            true,
            true,
        ),
        (5.0, None, PointerType::Touch, true, true),
        (
            15.0,
            Some(PointerButton::Primary),
            PointerType::Mouse,
            true,
            false,
        ),
        (
            25.0,
            Some(PointerButton::Primary),
            PointerType::Mouse,
            true,
            false,
        ),
        (
            5.0,
            Some(PointerButton::Secondary),
            PointerType::Mouse,
            true,
            false,
        ),
        (
            5.0,
            Some(PointerButton::Primary),
            PointerType::Mouse,
            false,
            false,
        ),
    ] {
        let mut frame = drag_frame(
            gesture_place(
                crate::display::on_state_drag(
                    crate::display::row(0.0, []),
                    target(),
                    Rc::new(|_| true),
                    Rc::new(|| Box::new(|_, _| Value::record([]))),
                ),
                false,
            ),
            false,
        );
        let mut world = crate::test_editor(Document {
            root: None,
            cells: Cells::new(),
        });
        let mut event = press(x, false);
        event.button = button;
        event.pointer.pointer_type = pointer_type;
        let handled = frame.resolve_for_dispatch().dispatch_pointer_down_with(
            &mut world,
            &event,
            &mut placed::DispatchContext::new(owns_view.then(crate::test_root), Some(target())),
        );
        assert_eq!(handled, expected);
        assert_eq!(world.gesture.is_some(), expected);
    }
}

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
        let mut frame = drag_frame(
            gesture_place(
                crate::libraries::number::scrub::on_scrub(
                    crate::display::row(0.0, []),
                    target(),
                    Rc::new(|| {
                        Box::new(|_| crate::libraries::number::scrub::ScrubUpdate {
                            value: f64_convention::value(13.0),
                            spelling: Some("13".into()),
                        })
                    }),
                ),
                false,
            ),
            covered,
        );
        let mut world = crate::test_editor(Document {
            root: Some(f64_convention::value(1.0)),
            cells: Cells::new(),
        });
        if pending {
            world.model.selection = Some(pending_value(&crate::test_root(), vec![]));
        }
        let handled = frame.resolve_for_dispatch().dispatch_pointer_down_with(
            &mut world,
            &press(x, pick),
            &mut placed::DispatchContext::new(owns_view.then(crate::test_root), Some(target())),
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
    use crate::display::widget::{before_place, interaction::target_action};

    for (pending, raw) in [(false, false), (true, false), (false, true), (true, true)] {
        let field = new_cell_id();
        let value = f64_convention::value(1.0);
        let mut world = crate::test_editor(Document {
            root: Some(value.clone()),
            cells: Cells::new(),
        });
        if pending {
            world.model.selection =
                Some(pending_value(&crate::test_root(), vec![Step::Key(field)]));
        }
        let before = world.model.doc.clone();
        let raw_contacts = Rc::new(std::cell::Cell::new(0));
        let contacts = raw_contacts.clone();
        let scrub = gesture_place(
            crate::libraries::number::scrub::on_scrub(
                crate::display::row(0.0, []),
                target(),
                Rc::new(|| {
                    Box::new(|_| crate::libraries::number::scrub::ScrubUpdate {
                        value: f64_convention::value(13.0),
                        spelling: None,
                    })
                }),
            ),
            false,
        );
        let pick = target_action(
            target(),
            Rc::new(move |world: &mut crate::Editor| world.pick_identity(value.clone())),
            true,
            crate::editing::picking,
            PartialEq::eq,
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
            move |placement, output| pick(output, placement),
        );
        let mut frame = crate::display::widget::frame::place(
            placed::in_view(node, crate::test_root()),
            Placement::root(extent.rect_at(Point::ZERO)),
            &Default::default(),
        );
        assert!(frame.resolve_for_dispatch().dispatch_pointer_down_with(
            &mut world,
            &press(5.0, true),
            &mut placed::DispatchContext::new(Some(crate::test_root()), Some(target())),
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

#[test]
fn readonly_gesture_controls_do_not_start_or_construct_edit_runs() {
    for layout in [
        crate::libraries::number::scrub::on_scrub(
            crate::display::row(0.0, []),
            target(),
            Rc::new(|| panic!("read-only scrub")),
        ),
        crate::display::on_point(
            crate::display::row(0.0, []),
            Rc::new(|_| panic!("read-only point control")),
        ),
    ] {
        let frame = drag_frame(gesture_place(layout, true), false);
        let mut world = crate::test_editor(Document {
            root: None,
            cells: Cells::new(),
        });
        assert!(
            !frame
                .handler
                .is_some_and(|handler| handler.dispatch_pointer_down_with(
                    &mut world,
                    &press(5.0, true),
                    &mut placed::DispatchContext::new(Some(crate::test_root()), Some(target()))
                ))
        );
        assert!(world.gesture.is_none());
    }
}
