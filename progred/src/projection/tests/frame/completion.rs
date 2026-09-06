use super::*;

#[test]
fn completion_constructor_shortcuts_precede_query_input_even_in_a_narrow_picker() {
    struct Clipboard;
    impl puri::edit::TextClipboard for Clipboard {
        fn get_text(&mut self) -> Option<String> {
            None
        }
        fn set_text(&mut self, _: &str) {}
    }
    struct State {
        selection: Selection,
        committed: Vec<Value>,
        fonts: parley::FontContext,
        layouts: parley::LayoutContext<Brush>,
        clipboard: Clipboard,
    }

    let stack = crate::stack::load::<State>();
    let root = crate::workspace::Root::document();
    let empty = Document {
        root: None,
        cells: Cells::new(),
    };
    let record = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let mut state = State {
        selection: pending_value(&root, vec![]),
        committed: Vec::new(),
        fonts: parley::FontContext::new(),
        layouts: parley::LayoutContext::new(),
        clipboard: Clipboard,
    };
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
    let mut frame = |state: &State, doc: &Document| {
        let node = project::<State, Bench>(
            ProjectDescription {
                sources: src(doc, &stack.libraries),
                root: doc.root.as_ref(),
                root_path: &[],
                selection: Some(&state.selection),
                scrub_spelling: None,
                source_selection: None,
                annotations: &Annotations::default(),
                raw: false,
                styles: &styles,
                width: 600.0,
                projection: Some(&stack.projection),
            },
            &mut tcx,
            Hooks {
                completions: Some(stack.completions.clone()),
                select: Rc::new(|_, _| {}),
                select_payload: Rc::new(|_, _, _| {}),
                start_edit: Rc::new(|_, _, _| {}),
                toggle: Rc::new(|_, _| {}),
                update_state: Rc::new(|_, _, _| false),
                edit: Rc::new(|state: &mut State| {
                    Some(puri::edit::EditCtx {
                        state: state.selection.edit_mut()?,
                        fonts: &mut state.fonts,
                        layouts: &mut state.layouts,
                        clipboard: &mut state.clipboard,
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
                commit_value: Rc::new(|state, value, _| state.committed.push(value)),
                commit_label: Rc::new(|state, cell, _, _| state.committed.push(Value::from(cell))),
                set_completion_view: Rc::new(|state, scroll, choice, everything| {
                    state
                        .selection
                        .set_completion_view(scroll, choice, everything);
                }),
            },
        );
        let rect = node.extent.rect_at(Point::new(20.0, 20.0));
        measured::place(
            node,
            Placement::new(rect, Rect::new(0.0, 0.0, 640.0, 480.0)),
        )
        .raise_floaters()
    };
    let press = |key: &str, modifiers| KeyboardEvent {
        key: Key::Character(key.into()),
        state: KeyState::Down,
        modifiers,
        ..Default::default()
    };
    for labels in [false, true] {
        let doc = if labels { &record } else { &empty };
        for everything in [false, true] {
            for key in ["[", "(", "{"] {
                state.selection = if labels {
                    pending_edge(&root, &src(doc, &stack.libraries), vec![]).unwrap()
                } else {
                    pending_value(&root, vec![])
                };
                state.selection.set_completion_view(0.0, 0, everything);
                let placed = frame(&state, doc);
                if !everything {
                    assert!(
                        placed
                            .completion
                            .as_ref()
                            .unwrap()
                            .entries
                            .iter()
                            .all(|entry| !entry.display.starts_with("new "))
                    );
                }
                assert!(
                    placed
                        .handler
                        .unwrap()
                        .dispatch_key(&mut state, &press(key, Modifiers::SHIFT))
                );
                if labels && key != "(" {
                    assert!(state.committed.is_empty());
                    assert_eq!(state.selection.edit().unwrap().text(), key);
                } else {
                    let value = state.committed.pop().unwrap();
                    match key {
                        "[" => assert_eq!(value, Value::list([])),
                        "{" => assert_eq!(value, Value::record([])),
                        _ => assert!(value.as_cell().is_some()),
                    }
                    assert!(state.selection.edit().unwrap().text().is_empty());
                    assert_eq!(state.selection.completion_everything(), everything);
                }
            }
        }
    }

    state.selection = pending_value(&root, vec![]);
    for modifiers in [Modifiers::CONTROL, Modifiers::META] {
        assert!(
            !frame(&state, &empty)
                .handler
                .unwrap()
                .dispatch_key(&mut state, &press("[", modifiers))
        );
        assert!(state.committed.is_empty());
    }
    let release = KeyboardEvent {
        state: KeyState::Up,
        ..press("[", Modifiers::empty())
    };
    assert!(
        !frame(&state, &empty)
            .handler
            .unwrap()
            .dispatch_key(&mut state, &release)
    );
    assert!(state.committed.is_empty());
    assert!(
        frame(&state, &empty)
            .handler
            .unwrap()
            .dispatch_key(&mut state, &press("[", Modifiers::ALT))
    );
    assert_eq!(state.committed.pop(), Some(Value::list([])));

    for query in ["\"", "search"] {
        state.selection = crate::selection::pending_with_query(&root, vec![], query);
        assert!(
            frame(&state, &empty)
                .handler
                .unwrap()
                .dispatch_key(&mut state, &press("[", Modifiers::empty()))
        );
        assert!(state.committed.is_empty());
        assert_eq!(state.selection.edit().unwrap().text(), format!("{query}["));
    }
    state.selection = pending_value(&root, vec![]);
    state
        .selection
        .edit_mut()
        .unwrap()
        .handle_ime(&puri::handler::ImeEvent::Preedit("[".into(), Some((1, 1))));
    assert!(
        !frame(&state, &empty)
            .handler
            .unwrap()
            .dispatch_key(&mut state, &press("[", Modifiers::empty()))
    );
    assert!(state.committed.is_empty());
    assert!(
        frame(&state, &empty)
            .handler
            .unwrap()
            .dispatch_ime(&mut state, &puri::handler::ImeEvent::Commit("[".into()))
    );
    assert!(state.committed.is_empty());
    assert_eq!(state.selection.edit().unwrap().text(), "[");
}

#[test]
fn a_completion_without_an_edit_still_consumes_its_activation() {
    let entries = [Entry {
        display: "no edit".into(),
        detail: None,
        matches: vec![],
        face: progred_display::Face::Label,
        source: None,
        activate: Rc::new(|attempts: &mut usize| {
            *attempts += 1;
        }),
    }];
    let mut fonts = parley::FontContext::new();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let mut tcx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let card = measured::place_top_left(
        completion_card::<usize, Bench>(
            &mut tcx,
            &crate::styles::editor(1.0),
            &entries,
            0,
            0.0,
            true,
            |_, _, _, _| {},
        ),
        Point::ZERO,
    );
    let mut attempts = 0;
    assert!(card.handler.unwrap().dispatch_key(
        &mut attempts,
        &KeyboardEvent {
            key: Key::Named(NamedKey::Enter),
            state: KeyState::Down,
            ..Default::default()
        }
    ));
    assert_eq!(attempts, 1);
}

#[test]
fn completion_details_share_the_cards_right_edge() {
    for scale in [1.0, 2.0] {
        let mut context = BenchContext::new();
        let styles = crate::styles::editor(scale);
        let mut tcx = TextCtx {
            fonts: &mut context.fonts,
            layouts: &mut context.layouts,
            cache: &mut context.cache,
            scale: scale as f32,
        };
        let entries = [
            ("x", "a long library name"),
            ("a much longer label", "short"),
        ]
        .map(|(display, detail)| Entry {
            display: display.into(),
            detail: Some(detail.into()),
            matches: Vec::new(),
            face: progred_display::Face::Name,
            source: None,
            activate: Rc::new(|_: &mut ()| {}),
        });
        let detail_widths = entries.each_ref().map(|entry| {
            puri::text(&mut tcx, entry.detail.as_deref().unwrap(), &styles.dim)
                .metrics()
                .width
        });
        let card = completion_card::<(), Bench>(
            &mut tcx,
            &styles,
            &entries,
            0,
            0.0,
            true,
            |_, _, _, _| {},
        );
        let origin = Point::new(37.0, 59.0);
        let right = origin.x + card.extent.width - (4.0 + 8.0) * scale;
        let bench = settle(measured::place_top_left(card, origin), None);
        let details = bench
            .list
            .0
            .iter()
            .filter_map(|command| match command {
                DrawCmd::GlyphRun(run) if run.brush == styles.dim.brush => Some(run),
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
            face: progred_display::Face::String,
            source: None,
            activate: Rc::new(|_| {}),
        },
        Entry {
            display: "new list".to_string(),
            detail: None,
            matches: Vec::new(),
            face: progred_display::Face::Dim,
            source: None,
            activate: Rc::new(|_| {}),
        },
    ];
    let place_card = |pointer| {
        let mut fonts = parley::FontContext::new();
        let mut layouts = parley::LayoutContext::new();
        let mut cache = puri::text::TextCache::default();
        let mut tcx = TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            scale: 1.0,
            cache: &mut cache,
        };
        let card = completion_card::<World, Bench>(
            &mut tcx,
            &crate::styles::editor(1.0),
            &entries,
            0,
            0.0,
            false,
            |_, _, _, _| {},
        );
        let extent = card.extent;
        let placed = measured::place_top_left(card, Point::ZERO);
        (settle(placed, Some(pointer)), extent)
    };
    // The card's own padding claims-and-clears: an overlay's
    // pointer never falls through to what sits beneath it.
    let (padding, extent) = place_card(Point::new(1.0, 1.0));
    assert_eq!(padding.hit, Some(Claim::Occludes));
    // Scanning down the card crosses both rows, each claiming its
    // index — an address into the frame's exact visible offers.
    let winners: Vec<Hover> = (0..extent.height() as usize)
        .filter_map(|y| {
            let (bench, _) = place_card(Point::new(extent.width / 2.0, y as f64 + 0.5));
            match bench.hit {
                Some(Claim::Direct(Hovered::Tree(hover))) => Some(hover),
                _ => None,
            }
        })
        .collect();
    assert!(winners.contains(&Hover::Entry(0)));
    assert!(winners.contains(&Hover::Entry(1)));
    assert!(winners.contains(&Hover::MoreCompletions));
}

#[test]
fn completion_viewport_scrolls_without_losing_keyboard_reveal() {
    use ui_events::pointer::{PointerId, PointerInfo, PointerState};

    let entries: Vec<_> = (0..20)
        .map(|index| Entry {
            display: format!("entry {index}"),
            detail: None,
            matches: Vec::new(),
            face: progred_display::Face::Dim,
            source: None,
            activate: Rc::new(|_| {}),
        })
        .collect();
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
    let mut frame = |(scroll, choice, everything)| {
        measured::place_top_left(
            completion_card::<(f64, usize, bool), Bench>(
                &mut tcx,
                &styles,
                &entries,
                choice,
                scroll,
                everything,
                |state, scroll, choice, everything| *state = (scroll, choice, everything),
            ),
            Point::ZERO,
        )
    };
    let mut state = (0.0, 0, false);
    assert_eq!(
        frame(state).probe(Point::new(10.0, 10.0), None, 0.0),
        Some(Claim::Direct(Hovered::Tree(Hover::Entry(0)))),
    );
    let pointer = PointerInfo {
        pointer_id: Some(PointerId::PRIMARY),
        persistent_device_id: None,
        pointer_type: PointerType::Mouse,
    };
    let mut pointer_state = PointerState::default();
    pointer_state.position.x = 10.0;
    pointer_state.position.y = 10.0;
    let scroll = PointerScrollEvent {
        pointer,
        state: pointer_state,
        delta: ScrollDelta::LineDelta(0.0, -5.0),
    };
    assert!(
        frame(state)
            .handler
            .unwrap()
            .dispatch_scroll(&mut state, &scroll)
            .handled()
    );
    assert_eq!(state, (200.0, 0, false));
    let scrolled = frame(state);
    assert!((4..160).any(|y| matches!(
        scrolled.probe(Point::new(10.0, y as f64), None, 0.0),
        Some(Claim::Direct(Hovered::Tree(Hover::Entry(index)))) if index > 0
    )));
    assert!(
        (0..200).all(|y| scrolled.probe(Point::new(10.0, y as f64), None, 0.0)
            != Some(Claim::Direct(Hovered::Tree(Hover::Entry(0)))))
    );
    let press = |key| KeyboardEvent {
        key: Key::Named(key),
        state: KeyState::Down,
        ..Default::default()
    };
    for key in [NamedKey::ArrowDown, NamedKey::ArrowUp] {
        assert!(
            frame(state)
                .handler
                .unwrap()
                .dispatch_key(&mut state, &press(key))
        );
    }
    assert_eq!(state, (0.0, 0, false));
    for _ in 0..12 {
        frame(state)
            .handler
            .unwrap()
            .dispatch_key(&mut state, &press(NamedKey::ArrowDown));
    }
    let offset = state.0;
    assert!(offset > 0.0);
    frame(state)
        .handler
        .unwrap()
        .dispatch_key(&mut state, &press(NamedKey::ArrowUp));
    assert_eq!(state, (offset, 11, false));
    for _ in 11..entries.len() {
        frame(state)
            .handler
            .unwrap()
            .dispatch_key(&mut state, &press(NamedKey::ArrowDown));
    }
    assert_eq!(state.1, entries.len());
    let bottom = frame(state);
    assert!((0..200).any(|y| {
        bottom.probe(Point::new(10.0, y as f64), None, 0.0)
            == Some(Claim::Direct(Hovered::Tree(Hover::MoreCompletions)))
    }));
    frame(state)
        .handler
        .unwrap()
        .dispatch_key(&mut state, &press(NamedKey::ArrowUp));
    assert_eq!(state.1, entries.len() - 1);
    frame(state)
        .handler
        .unwrap()
        .dispatch_key(&mut state, &press(NamedKey::ArrowDown));
    let before_expansion = state;
    frame(state)
        .handler
        .unwrap()
        .dispatch_key(&mut state, &press(NamedKey::Enter));
    assert_eq!(state, (before_expansion.0, before_expansion.1, true));
    frame(state)
        .handler
        .unwrap()
        .dispatch_key(&mut state, &press(NamedKey::ArrowDown));
    assert_eq!(state.1, entries.len() - 1);
    let before_tab = state;
    state.2 = false;
    frame(state)
        .handler
        .unwrap()
        .dispatch_key(&mut state, &press(NamedKey::Tab));
    assert_eq!(state, before_tab);
    assert!(
        !frame(state)
            .handler
            .unwrap()
            .dispatch_key(&mut state, &press(NamedKey::Tab))
    );
    assert_eq!(state, before_tab);
}

#[test]
fn completion_rows_activate_their_own_action_by_keyboard_or_pointer() {
    #[derive(Default)]
    struct State {
        view: (f64, usize, bool),
        committed: Option<Value>,
    }

    let entries = [Entry {
        display: "new list".into(),
        detail: None,
        matches: Vec::new(),
        face: progred_display::Face::Dim,
        source: None,
        activate: Rc::new(|state: &mut State| {
            state.committed = Some(Value::list([]));
        }),
    }];
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
    let mut frame = |state: &State, entries: &[Entry<State>]| {
        measured::place_top_left(
            completion_card::<State, Bench>(
                &mut tcx,
                &styles,
                entries,
                state.view.1,
                state.view.0,
                state.view.2,
                |state, scroll, choice, everything| state.view = (scroll, choice, everything),
            ),
            Point::ZERO,
        )
    };
    let press = |key| KeyboardEvent {
        key: Key::Named(key),
        state: KeyState::Down,
        ..Default::default()
    };
    let mut state = State::default();
    assert!(
        frame(&state, &entries)
            .handler
            .unwrap()
            .dispatch_key(&mut state, &press(NamedKey::Enter))
    );
    assert_eq!(state.committed.take(), Some(Value::list([])));
    assert!(!state.view.2);
    frame(&state, &entries)
        .handler
        .unwrap()
        .dispatch_key(&mut state, &press(NamedKey::ArrowDown));
    assert_eq!(state.view, (0.0, 1, false));
    for key in [NamedKey::Enter, NamedKey::ArrowDown] {
        state.view.2 = false;
        assert!(
            frame(&state, &entries)
                .handler
                .unwrap()
                .dispatch_key(&mut state, &press(key))
        );
        assert_eq!(state.view, (0.0, 1, true));
        assert!(state.committed.is_none());
    }
    let expanded = [
        entries[0].clone(),
        Entry {
            display: "new record".into(),
            activate: Rc::new(|state: &mut State| {
                state.committed = Some(Value::record([]));
            }),
            ..entries[0].clone()
        },
    ];
    assert!(
        frame(&state, &expanded)
            .handler
            .unwrap()
            .dispatch_key(&mut state, &press(NamedKey::Enter))
    );
    assert_eq!(state.committed.take(), Some(Value::record([])));
    assert!(
        frame(&state, &entries)
            .handler
            .unwrap()
            .dispatch_key(&mut state, &press(NamedKey::Enter))
    );
    assert_eq!(state.committed.take(), Some(Value::list([])));
    frame(&state, &entries)
        .handler
        .unwrap()
        .dispatch_key(&mut state, &press(NamedKey::ArrowDown));
    assert_eq!(state.view.1, 0);

    state.view = (0.0, 1, false);
    let placed = frame(&state, &entries);
    let target = Hovered::Tree(Hover::MoreCompletions);
    let point = (0..100)
        .map(|y| Point::new(10.0, y as f64))
        .find(|point| placed.probe(*point, None, 0.0) == Some(Claim::Direct(target.clone())))
        .expect("expansion row is visible");
    let mut pointer_state = ui_events::pointer::PointerState::default();
    pointer_state.position.x = point.x;
    pointer_state.position.y = point.y;
    let event = ui_events::pointer::PointerButtonEvent {
        button: Some(PointerButton::Primary),
        pointer: ui_events::pointer::PointerInfo {
            pointer_id: Some(ui_events::pointer::PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        },
        state: pointer_state,
    };
    assert!(placed.handler.unwrap().dispatch_pointer_down_with(
        &mut state,
        &event,
        &mut placed::DispatchContext::new(None, Some(target)),
    ));
    assert_eq!(state.view, (0.0, 1, true));
    assert!(state.committed.is_none());

    state.view = (0.0, 0, false);
    assert!(
        frame(&state, &[])
            .handler
            .unwrap()
            .dispatch_key(&mut state, &press(NamedKey::Enter))
    );
    assert_eq!(state.view, (0.0, 0, true));
    assert!(state.committed.is_none());
    assert!(
        !frame(&state, &[])
            .handler
            .unwrap()
            .dispatch_key(&mut state, &press(NamedKey::Enter))
    );
}

#[test]
fn completion_activation_precedes_the_real_editor_it_covers() {
    struct ClickWorld {
        doc: Document,
        libraries: Libraries,
        selection: Option<Selection>,
        applied: Option<Path>,
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
            select: Rc::new(|_, _| {}),
            select_payload: Rc::new(|_, _, _| {}),
            start_edit: Rc::new(|world: &mut ClickWorld, path, line| {
                world.selection = Some(Selection::from_line(
                    &crate::workspace::Root::document(),
                    &Sources {
                        doc: &world.doc,
                        libraries: &world.libraries,
                    },
                    path,
                    line,
                ));
            }),
            toggle: Rc::new(|_, _| {}),
            update_state: Rc::new(|_, _, _| false),
            // A selection transition must consume the click even if
            // retained dispatch cannot recover an edit context for
            // the optional caret-placement follow-up.
            edit: Rc::new(|_| None),
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
    let card = completion_card::<ClickWorld, Bench>(
        &mut tcx,
        &styles,
        &[Entry {
            display: "completion offer".into(),
            detail: None,
            matches: Vec::new(),
            face: progred_display::Face::String,
            source: None,
            activate: Rc::new(|world: &mut ClickWorld| {
                world.applied = Some(Vec::new());
            }),
        }],
        0,
        0.0,
        true,
        |_, _, _, _| {},
    );
    let card_rect = card
        .extent
        .rect_at(Point::new(point.x - 8.0, point.y - 8.0));
    let card = measured::place(card, Placement::root(card_rect));
    let placed = measured::Output::over(placed, card);
    let Some(Claim::Direct(target)) = placed.probe(point, None, 0.0) else {
        panic!("direct hover")
    };
    assert_eq!(target, Hovered::Tree(Hover::Entry(0)));
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
    };
    let mut pointer = placed::DispatchContext::new(None, Some(target));
    assert!(placed.handler.as_ref().unwrap().dispatch_pointer_down_with(
        &mut world,
        &event,
        &mut pointer
    ));
    assert!(world.selection.is_none());
    assert_eq!(world.applied, Some(Vec::new()));
}
