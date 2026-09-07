use super::*;
use progred_libraries::f64 as f64_convention;

#[test]
fn sample_text_line_click_mounts_its_own_editor() {
    #[derive(Default)]
    struct Clipboard(Option<String>);

    impl puri::edit::TextClipboard for Clipboard {
        fn get_text(&mut self) -> Option<String> {
            self.0.clone()
        }

        fn set_text(&mut self, text: &str) {
            self.0 = Some(text.to_string());
        }
    }

    struct ClickWorld {
        doc: Document,
        libraries: Libraries,
        selection: Option<Selection>,
        applied: Option<Path>,
        fonts: parley::FontContext,
        layouts: parley::LayoutContext<Brush>,
        clipboard: Clipboard,
    }

    let (doc, _) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/sample.gid"
    )))
    .expect("the sample parses");
    let stack = crate::stack::load::<ClickWorld>();
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
    let node = project::<ClickWorld, Bench>(
        ProjectDescription {
            sources: Sources {
                doc: &doc,
                libraries: &stack.libraries,
            },
            root: doc.root.as_ref(),
            root_path: &[],
            selection: None,
            scrub_spelling: None,
            source_selection: None,
            annotations: &Annotations::default(),
            raw: false,
            styles: &styles,
            width: 852.0,

            projection: Some(&stack.projection),
        },
        &mut tcx,
        Hooks {
            completions: Some(stack.completions.clone()),
            select: Rc::new(|world: &mut ClickWorld, path| {
                world.selection = Some(make_selection(path));
            }),
            select_payload: Rc::new(|_, _, _| {}),
            edit_line: Rc::new(|_, _, _, _| false),
            toggle: Rc::new(|_, _| {}),
            update_state: Rc::new(|_, _, _| false),
            // A selection transition must consume the click even if
            // retained dispatch cannot recover an edit context for
            // the optional caret-placement follow-up.
            edit: Rc::new(|_, _| false),
            pick: Rc::new(|_, _| false),
            insert: Rc::new(|_, _| {}),
            delete: Rc::new(|_, _| false),
            apply: Rc::new(|world: &mut ClickWorld, path, _, _| {
                world.applied = Some(path);
                true
            }),
            point: Rc::new(|_, _, _, _, _| false),
            state_drag: Rc::new(|_, _, _, _, _| {}),
            scrub: Rc::new(|_, _, _, _, _| false),
            select_source: Rc::new(|_, _, _| {}),
            commit_value: Rc::new(|_, _, _| {}),
            commit_label: Rc::new(|_, _, _, _| {}),
            set_completion_view: Rc::new(|_, _, _, _| {}),
        },
    );
    let path = vec![
        Step::Key(sample_vocabulary::STYLE),
        Step::Follow(gid::Resolution::Document),
        Step::Key(sample_vocabulary::COLOR),
    ];
    let rect = node.extent.rect_at(Point::new(24.0, 24.0));
    let placed = measured::place(node, Placement::root(rect));
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
    let mut world = ClickWorld {
        doc: doc.clone(),
        libraries: stack.libraries.clone(),
        selection: None,
        applied: None,
        fonts: parley::FontContext::new(),
        layouts: parley::LayoutContext::new(),
        clipboard: Clipboard::default(),
    };
    assert!(
        placed
            .handler
            .expect("line handler")
            .dispatch_pointer_down(&mut world, &event)
    );
    assert_eq!(
        world.selection.as_ref().map(|selection| selection.path()),
        Some(path.as_slice())
    );
    assert_eq!(world.applied, None);

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
    let active = project::<ClickWorld, Bench>(
        ProjectDescription {
            sources: Sources {
                doc: &world.doc,
                libraries: &world.libraries,
            },
            root: world.doc.root.as_ref(),
            root_path: &[],
            selection: world.selection.as_ref(),
            scrub_spelling: None,
            source_selection: world.selection.as_ref(),
            annotations: &Annotations::default(),
            raw: false,
            styles: &styles,
            width: 852.0,

            projection: Some(&stack.projection),
        },
        &mut frame_tcx,
        Hooks {
            completions: Some(stack.completions.clone()),
            select: Rc::new(|_, _| {}),
            select_payload: Rc::new(|_, _, _| {}),
            edit_line: Rc::new(|world: &mut ClickWorld, path, line, operation| {
                let Some(selected) = world
                    .selection
                    .as_mut()
                    .filter(|selected| selected.path() == path)
                else {
                    return false;
                };
                operation(EditCtx {
                    state: selected.edit_line_mut(&line.text),
                    fonts: &mut world.fonts,
                    layouts: &mut world.layouts,
                    clipboard: &mut world.clipboard,
                })
            }),
            toggle: Rc::new(|_, _| {}),
            update_state: Rc::new(|_, _, _| false),
            edit: Rc::new(|world: &mut ClickWorld, operation| {
                let ClickWorld {
                    selection,
                    fonts,
                    layouts,
                    clipboard,
                    ..
                } = world;
                selection.as_mut().is_some_and(|selection| {
                    selection.edit_query(|state| {
                        operation(puri::edit::EditCtx {
                            state,
                            fonts,
                            layouts,
                            clipboard,
                        })
                    })
                })
            }),
            pick: Rc::new(|_, _| false),
            insert: Rc::new(|_, _| {}),
            delete: Rc::new(|_, _| false),
            apply: Rc::new(|_, _, _, _| false),
            point: Rc::new(|_, _, _, _, _| false),
            state_drag: Rc::new(|_, _, _, _, _| {}),
            scrub: Rc::new(|_, _, _, _, _| false),
            select_source: Rc::new(|_, _, _| {}),
            commit_value: Rc::new(|_, _, _| {}),
            commit_label: Rc::new(|_, _, _, _| {}),
            set_completion_view: Rc::new(|_, _, _, _| {}),
        },
    );
    let active = measured::place(active, Placement::root(rect));
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
    let handler = active.handler.expect("active line handler");
    assert!(handler.dispatch_pointer_down(&mut world, &double));
    let selection = world.selection.as_ref().unwrap().edit().unwrap();
    let (anchor, focus) = selection.selection_offsets();
    assert_ne!(anchor, focus);

    let mut single = double;
    single.state.position.x = line.x0 + 1.0;
    single.state.count = 1;
    assert!(handler.dispatch_pointer_down(&mut world, &single));
    let selection = world.selection.as_ref().unwrap().edit().unwrap();
    let (anchor, focus) = selection.selection_offsets();
    assert_eq!(anchor, focus);
}

#[test]
fn state_drag_press_composes_selection_and_start_in_pointer_order() {
    use ui_events::pointer::{
        PointerButtonEvent, PointerId, PointerInfo, PointerState, PointerType,
    };

    let target = Hover::Value(Rc::from([]));
    let path = vec![Step::Key(gid::new_cell_id())];
    let extent = Extent {
        width: 20.0,
        ascent: 0.0,
        descent: 20.0,
    };
    for (accepts, covered) in [(true, false), (false, false), (true, true)] {
        let captured_path = path.clone();
        let drag = realize_state_drag(
            path.clone(),
            target.clone(),
            Rc::new(move |log: &mut Vec<&str>| {
                log.push("select");
                accepts
            }),
            Rc::new(|| Box::new(|_, _| Value::record([]))),
            Rc::new(move |log, path, _, point, scale| {
                assert_eq!(path, captured_path);
                assert_eq!(point, Point::new(5.0, 5.0));
                assert_eq!(scale, 2.0);
                log.push("start drag");
            }),
            2.0,
            leaf::<Vec<&str>, Bench>(extent, |_, _| {}),
        );
        let node = realize_activate(
            target.clone(),
            Rc::new(|log: &mut Vec<&str>| {
                log.push("outer selection");
                true
            }),
            drag,
        );
        let placement = Placement::root(Rect::new(0.0, 0.0, 20.0, 20.0));
        let mut placed = measured::place(node, placement);
        if covered {
            let cover = leaf::<Vec<&str>, Bench>(extent, |p, placement| {
                p.occlude(placement);
            });
            placed = measured::Output::over(placed, measured::place(cover, placement));
        }
        let mut state = PointerState::default();
        state.position.x = 5.0;
        state.position.y = 5.0;
        let event = PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state,
        };
        let mut pointer = placed::DispatchContext::new(None, Some(Hovered::Tree(target.clone())));
        let mut log = Vec::new();
        assert!(
            placed
                .handler
                .unwrap()
                .dispatch_pointer_down_with(&mut log, &event, &mut pointer)
        );
        assert_eq!(
            log,
            if covered {
                vec![]
            } else if accepts {
                vec!["select", "start drag"]
            } else {
                vec!["select", "outer selection"]
            }
        );
    }
}

#[test]
fn state_drag_starts_only_at_a_visible_primary_contact_in_its_own_view() {
    use ui_events::pointer::{
        PointerButtonEvent, PointerId, PointerInfo, PointerState, PointerType,
    };

    let target = Hover::Value(Rc::from([]));
    let root = crate::workspace::Root::document();
    let node = realize_state_drag(
        Vec::new(),
        target.clone(),
        Rc::new(|_| true),
        Rc::new(|| Box::new(|_, _| Value::record([]))),
        Rc::new(|starts: &mut usize, _, _, _, _| *starts += 1),
        1.0,
        leaf::<usize, Bench>(
            Extent {
                width: 20.0,
                ascent: 0.0,
                descent: 20.0,
            },
            |_, _| {},
        ),
    );
    let placed = measured::place(
        placed::in_view(node, root.clone()),
        Placement::new(
            Rect::new(0.0, 0.0, 20.0, 20.0),
            Rect::new(0.0, 0.0, 10.0, 20.0),
        ),
    );
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
        let mut state = PointerState::default();
        state.position.x = x;
        state.position.y = 5.0;
        let event = PointerButtonEvent {
            button,
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type,
            },
            state,
        };
        let mut pointer = placed::DispatchContext::new(
            owns_view.then(|| root.clone()),
            Some(Hovered::Tree(target.clone())),
        );
        let mut starts = 0;
        assert_eq!(
            placed.handler.as_ref().unwrap().dispatch_pointer_down_with(
                &mut starts,
                &event,
                &mut pointer,
            ),
            expected
        );
        assert_eq!(starts, usize::from(expected));
    }
}

#[test]
fn scrub_start_respects_dispatch_order_pending_picks_and_visible_view_geometry() {
    use ui_events::keyboard::Modifiers;
    use ui_events::pointer::{
        PointerButtonEvent, PointerId, PointerInfo, PointerState, PointerType,
    };

    struct World {
        pending: bool,
        log: Vec<&'static str>,
        scrub: Option<Box<dyn crate::gesture::Gesture>>,
    }

    let path = vec![Step::Key(new_cell_id())];
    let target = Hover::Value(Rc::from(path.clone()));
    let root = crate::workspace::Root::document();
    let value = f64_convention::value(12.0);
    let extent = Extent {
        width: 20.0,
        ascent: 0.0,
        descent: 20.0,
    };
    for (pending, covered, raw, x, pick, owns_view, expected) in [
        (false, false, false, 5.0, true, true, Some("scrub")),
        (true, false, false, 5.0, true, true, Some("pending pick")),
        (false, true, false, 5.0, true, true, None),
        (false, false, true, 5.0, true, true, Some("raw")),
        (false, false, false, 15.0, true, true, None),
        (false, false, false, 5.0, false, true, None),
        (false, false, false, 5.0, true, false, None),
    ] {
        let captured_root = root.clone();
        let scrub = realize_scrub(
            path.clone(),
            target.clone(),
            Rc::new(|| {
                Box::new(|_| progred_display::ScrubUpdate {
                    value: f64_convention::value(13.0),
                    spelling: Some("13".into()),
                })
            }),
            Rc::new(move |world: &mut World, path, handler, point, scale| {
                if world.pending {
                    false
                } else {
                    world.log.push("scrub");
                    world.scrub = Some(crate::gesture::scrub(
                        point,
                        scale,
                        captured_root.clone(),
                        path,
                        handler,
                    ));
                    true
                }
            }),
            2.0,
            leaf::<World, Bench>(extent, move |p, _| {
                p.handler().on_pointer_down(move |world, _| {
                    if raw {
                        world.log.push("raw");
                    }
                    raw
                });
            }),
        );
        let picked = value.clone();
        let node = realize_pick_with(
            target.clone(),
            value.clone(),
            Rc::new(move |world: &mut World, value| {
                assert_eq!(value, picked);
                if world.pending {
                    world.log.push("pending pick");
                }
                world.pending
            }),
            scrub,
        );
        let placement = Placement::new(
            Rect::new(0.0, 0.0, 20.0, 20.0),
            Rect::new(0.0, 0.0, 10.0, 20.0),
        );
        let mut placed = measured::place(placed::in_view(node, root.clone()), placement);
        if covered {
            placed = measured::Output::over(
                placed,
                measured::place(leaf(extent, |p, placement| p.occlude(placement)), placement),
            );
        }
        let mut state = PointerState::default();
        state.position.x = x;
        state.position.y = 5.0;
        state.modifiers = if pick {
            Modifiers::META | Modifiers::CONTROL
        } else {
            Modifiers::empty()
        };
        let event = PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state,
        };
        let mut pointer = placed::DispatchContext::new(
            owns_view.then(|| root.clone()),
            Some(Hovered::Tree(target.clone())),
        );
        let mut world = World {
            pending,
            log: Vec::new(),
            scrub: None,
        };
        assert_eq!(
            placed
                .handler
                .unwrap()
                .dispatch_pointer_down_with(&mut world, &event, &mut pointer),
            covered || expected.is_some(),
        );
        assert_eq!(world.log, expected.into_iter().collect::<Vec<_>>());
        assert_eq!(world.scrub.is_some(), expected == Some("scrub"));
    }
}
