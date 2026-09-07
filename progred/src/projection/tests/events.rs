use super::*;

#[test]
fn state_scroll_acceptance_does_not_depend_on_a_changed_value() {
    let event = PointerScrollEvent {
        pointer: PointerInfo {
            pointer_id: Some(PointerId::PRIMARY),
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        },
        state: PointerState::default(),
        delta: ScrollDelta::LineDelta(0.0, 1.0),
    };
    for accepts in [false, true] {
        let inner = leaf::<usize, crate::frame::Paint>(
            Extent {
                width: 20.0,
                ascent: 10.0,
                descent: 10.0,
            },
            |_, _| {},
        );
        let layout = events::realize_state_scroll(
            vec![],
            Rc::new(move |event| {
                (
                    accepts.then(|| Value::record([])),
                    if accepts {
                        puri::handler::ScrollOutcome::with_remainder(Default::default())
                    } else {
                        puri::handler::ScrollOutcome::unhandled(event)
                    },
                )
            }),
            Rc::new(|writes, _, _| {
                *writes += 1;
                false
            }),
            1.0,
            inner,
        );
        let placed = measured::place(layout, Placement::root(Rect::new(0.0, 0.0, 20.0, 20.0)));
        let mut writes = 0;
        let outcome = placed.handler.unwrap().dispatch_scroll(&mut writes, &event);
        assert_eq!(outcome.handled(), accepts);
        assert_eq!(writes, usize::from(accepts));
        match outcome.remaining {
            None => assert!(accepts),
            Some(puri::handler::Event::Scroll(remaining)) => {
                assert!(!accepts);
                assert_eq!(remaining.delta, event.delta);
            }
            _ => panic!("unexpected scroll remainder"),
        }
    }
}

#[test]
fn state_scroll_preserves_partial_consumption_and_units() {
    use progred_display::StateScrollEvent;
    use puri::handler::ScrollOutcome;

    for delta in [
        ScrollDelta::LineDelta(2.0, 4.0),
        ScrollDelta::PageDelta(2.0, 4.0),
        ScrollDelta::PixelDelta((2.0, 4.0).into()),
    ] {
        let event = PointerScrollEvent {
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state: PointerState::default(),
            delta,
        };
        let expected = match delta {
            ScrollDelta::LineDelta(..) => StateScrollEvent {
                delta_x: 80.0,
                delta_y: 160.0,
            },
            ScrollDelta::PageDelta(..) => StateScrollEvent {
                delta_x: 20.0,
                delta_y: 60.0,
            },
            ScrollDelta::PixelDelta(..) => StateScrollEvent {
                delta_x: 1.0,
                delta_y: 2.0,
            },
        };
        let layout = events::realize_state_scroll(
            vec![],
            Rc::new(move |input| {
                assert_eq!(input, expected);
                (
                    None,
                    ScrollOutcome::with_remainder(StateScrollEvent {
                        delta_x: input.delta_x,
                        delta_y: input.delta_y / 2.0,
                    }),
                )
            }),
            Rc::new(|_: &mut (), _, _| panic!("acceptance does not require a state write")),
            2.0,
            leaf::<(), crate::frame::Paint>(
                Extent {
                    width: 20.0,
                    ascent: 15.0,
                    descent: 15.0,
                },
                |_, _| {},
            ),
        );
        let placed = measured::place(layout, Placement::root(Rect::new(0.0, 0.0, 20.0, 30.0)));
        let outcome = placed.handler.unwrap().dispatch_scroll(&mut (), &event);
        assert!(outcome.handled());
        let Some(puri::handler::Event::Scroll(remaining)) = outcome.remaining else {
            panic!("expected unconsumed scroll")
        };
        assert_eq!(
            remaining.delta,
            match delta {
                ScrollDelta::LineDelta(..) => ScrollDelta::LineDelta(2.0, 2.0),
                ScrollDelta::PageDelta(..) => ScrollDelta::PageDelta(2.0, 2.0),
                ScrollDelta::PixelDelta(..) => ScrollDelta::PixelDelta((2.0, 2.0).into()),
            }
        );
    }
}

#[test]
fn a_data_event_realizes_the_apply_hook() {
    fn probe(
        input: &progred_display::ProjectionInput<'_, Vec<(Path, Value, Value)>, Hover>,
    ) -> Option<progred_display::Layout<Vec<(Path, Value, Value)>, Hover>> {
        use progred_libraries::layout as data;
        input.value?.as_blob()?;
        let target = input.targets.current();
        data::decode(
            &data::on(
                data::text_leaf("go", data::vocabulary::NAME_FACE),
                Value::from(data::vocabulary::HANDLER),
            ),
            &target.select,
            &target.hover,
        )
    }
    let doc = Document {
        root: Some(Value::from(vec![7u8])),
        cells: Cells::new(),
    };
    let lib = core_libraries();
    let projection: Projection<Vec<(Path, Value, Value)>> =
        Projection::new([progred_display::partial(probe)]);
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
    let empty = Annotations::default();
    let measured = project::<Vec<(Path, Value, Value)>, crate::frame::Paint>(
        ProjectDescription {
            sources: Sources {
                doc: &doc,
                libraries: &lib,
            },
            root: doc.root.as_ref(),
            root_path: &[],
            selection: None,
            scrub_spelling: None,
            source_selection: None,
            annotations: &empty,
            raw: false,
            styles: &styles,
            width: 500.0,

            projection: Some(&projection),
        },
        &mut tcx,
        Hooks::<Vec<(Path, Value, Value)>> {
            completions: None,
            select: Rc::new(|_, _| {}),
            select_payload: Rc::new(|_, _, _| {}),
            edit_line: Rc::new(|_, _, _, _| false),
            toggle: Rc::new(|_, _| {}),
            update_state: Rc::new(|_, _, _| false),
            edit: Rc::new(|_, _| false),
            pick: Rc::new(|_, _| false),
            insert: Rc::new(|_, _| {}),
            delete: Rc::new(|_, _| false),
            apply: Rc::new(|events, path, handler, event| {
                events.push((path, handler, event));
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
    assert!(measured.extent.width > 0.0);
    let placed = measured::place(
        measured,
        puri::geometry::Placement::root(measured_rect(500.0)),
    );
    let handler = placed.handler.expect("event handler");
    let mut state = PointerState::default();
    state.position.x = 1.0;
    state.position.y = 1.0;
    let mut events = Vec::new();
    assert!(handler.dispatch_pointer_down(
        &mut events,
        &PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state: state.clone(),
        },
    ));
    let [(path, function, event)] = &events[..] else {
        panic!("one event");
    };
    assert!(path.is_empty());
    assert_eq!(
        function,
        &Value::from(progred_libraries::layout::vocabulary::HANDLER)
    );
    assert_eq!(
        event
            .as_record()
            .and_then(|fields| fields.get(&progred_libraries::layout::vocabulary::EVENT_KIND))
            .and_then(Value::as_cell),
        Some(progred_libraries::layout::vocabulary::POINTER_DOWN),
    );
    assert!(handler.dispatch_pointer_move(
        &mut events,
        &PointerUpdate {
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            current: state.clone(),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        },
    ));
    assert_eq!(
        events[1]
            .2
            .as_record()
            .and_then(|fields| fields.get(&progred_libraries::layout::vocabulary::EVENT_KIND))
            .and_then(Value::as_cell),
        Some(progred_libraries::layout::vocabulary::POINTER_MOVE),
    );

    let touch = PointerInfo {
        pointer_id: PointerId::new(7),
        persistent_device_id: None,
        pointer_type: PointerType::Touch,
    };
    assert!(handler.dispatch_pointer_down(
        &mut events,
        &PointerButtonEvent {
            button: None,
            pointer: touch,
            state: state.clone(),
        },
    ));
    let touch_start = events[2].2.as_record().expect("touch start record");
    assert_eq!(
        touch_start
            .get(&progred_libraries::layout::vocabulary::EVENT_KIND)
            .and_then(Value::as_cell),
        Some(progred_libraries::layout::vocabulary::TOUCH_START),
    );
    assert!(!touch_start.contains_key(&progred_libraries::layout::vocabulary::BUTTON));

    assert!(handler.dispatch_pointer_move(
        &mut events,
        &PointerUpdate {
            pointer: touch,
            current: state.clone(),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        },
    ));
    assert_eq!(
        events[3]
            .2
            .as_record()
            .and_then(|fields| fields.get(&progred_libraries::layout::vocabulary::EVENT_KIND))
            .and_then(Value::as_cell),
        Some(progred_libraries::layout::vocabulary::TOUCH_MOVE),
    );

    assert!(handler.dispatch_pointer_up(
        &mut events,
        &PointerButtonEvent {
            button: None,
            pointer: touch,
            state: state.clone(),
        },
    ));
    assert_eq!(
        events[4]
            .2
            .as_record()
            .and_then(|fields| fields.get(&progred_libraries::layout::vocabulary::EVENT_KIND))
            .and_then(Value::as_cell),
        Some(progred_libraries::layout::vocabulary::TOUCH_END),
    );

    assert!(handler.dispatch_pointer_cancel(&mut events, &touch));
    assert_eq!(
        events[5]
            .2
            .as_record()
            .and_then(|fields| fields.get(&progred_libraries::layout::vocabulary::EVENT_KIND))
            .and_then(Value::as_cell),
        Some(progred_libraries::layout::vocabulary::TOUCH_CANCEL),
    );

    assert!(
        handler
            .dispatch_scroll(
                &mut events,
                &PointerScrollEvent {
                    pointer: PointerInfo {
                        pointer_id: Some(PointerId::PRIMARY),
                        persistent_device_id: None,
                        pointer_type: PointerType::Mouse,
                    },
                    delta: ScrollDelta::LineDelta(0.0, 1.0),
                    state,
                },
            )
            .handled()
    );
    assert_eq!(
        events[6]
            .2
            .as_record()
            .and_then(|fields| fields.get(&progred_libraries::layout::vocabulary::EVENT_KIND))
            .and_then(Value::as_cell),
        Some(progred_libraries::layout::vocabulary::SCROLL),
    );
}
