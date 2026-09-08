use super::*;

#[test]
fn native_annotation_handler_retains_the_projected_site() {
    let cell = new_cell_id();
    let path = vec![Step::Follow(gid::Resolution::Document)];
    let mut cells = Cells::new();
    cells.set_value(cell, Value::from(vec![7u8]));
    let doc = Document {
        root: Some(cell.into()),
        cells,
    };
    let libraries = Libraries::default();
    let styles = crate::styles::editor(1.0);
    let projection = Projection::new([progred_display::partial(|input| {
        input.value?.as_blob()?;
        Some(progred_display::widget::before(
            progred_display::text("scroll"),
            Rc::new(|context| {
                let annotate = (context.annotate)();
                progred_display::widget::scroll::scroll(
                    context.styles.scale,
                    move |world, delta| {
                        annotate(world, f64::value(delta.y));
                        puri::handler::ScrollOutcome::with_remainder(Default::default())
                    },
                )
            }),
        ))
    })]);
    let mut fonts = parley::FontContext::new();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::TextCache::default();
    let measured = project::<Vec<(Path, Value)>>(
        ProjectDescription {
            sources: Sources {
                doc: &doc,
                libraries: &libraries,
            },
            root: doc.cells.value(cell),
            root_path: &path,
            selection: None,
            scrub_spelling: None,
            source_selection: None,
            annotations: &Annotations::default(),
            raw: false,
            styles: &styles,
            width: 500.0,
            projection: Some(&projection),
        },
        &mut TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            cache: &mut cache,
            scale: 1.0,
        },
        Hooks {
            completions: None,
            select: Rc::new(|_, _| {}),
            select_payload: Rc::new(|_, _, _| {}),
            edit_line: Rc::new(|_, _, _, _| panic!("scroll does not edit text")),
            toggle: Rc::new(|_, _| {}),
            update_state: Rc::new(|writes: &mut Vec<(Path, Value)>, path, state| {
                writes.push((path, state));
                false
            }),
            edit: Rc::new(|_, _| false),
            pick: Rc::new(|_, _| false),
            insert: Rc::new(|_, _| {}),
            delete: Rc::new(|_, _| false),
            apply: Rc::new(|_, _, _, _| panic!("native scroll does not interpret Grap")),
            start_gesture: Rc::new(|_, _, _, _| {}),
            value_edit: Rc::new(|_| panic!("unexpected value edit")),
            select_source: Rc::new(|_, _, _| {}),
            commit_value: Rc::new(|_, _, _| {}),
            commit_label: Rc::new(|_, _, _, _| {}),
            set_completion_view: Rc::new(|_, _, _, _| {}),
        },
    );
    let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
    let placed = measured::place(measured, placement).run(&Default::default());
    let mut writes = vec![];
    let event = PointerScrollEvent {
        pointer: PointerInfo {
            pointer_id: None,
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        },
        state: PointerState {
            position: (placement.rect.center().x, placement.rect.center().y).into(),
            ..Default::default()
        },
        delta: ScrollDelta::LineDelta(0.0, 1.0),
    };
    let outcome = placed.handler.unwrap().dispatch_scroll(&mut writes, &event);
    assert!(outcome.handled());
    assert!(outcome.remaining.is_none());
    assert_eq!(writes, vec![(path, f64::value(40.0))]);
}

#[test]
fn a_data_event_realizes_the_apply_hook() {
    fn probe(
        input: &progred_display::ProjectionInput<'_, Vec<(Path, Value, Value)>, Hovered>,
    ) -> Option<progred_display::Layout<Vec<(Path, Value, Value)>, Hovered>> {
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
    let measured = project::<Vec<(Path, Value, Value)>>(
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
            start_gesture: Rc::new(|_, _, _, _| {}),
            value_edit: Rc::new(|_| panic!("unexpected value edit")),
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
    )
    .run(&Default::default());
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

    use progred_libraries::layout::vocabulary as event_fields;
    let mut key = KeyboardEvent::default();
    key.modifiers = if cfg!(target_os = "macos") {
        Modifiers::META
    } else {
        Modifiers::CONTROL
    };
    assert!(handler.dispatch_key(&mut events, &key));
    let key_fields = events[7].2.as_record().unwrap();
    assert_eq!(
        key_fields.get(&event_fields::EVENT_KIND),
        Some(&Value::Cell(event_fields::KEY))
    );
    assert_eq!(
        key_fields.get(&event_fields::MODIFIERS),
        Some(&Value::list([Value::Cell(event_fields::COMMAND)]))
    );

    assert!(handler.dispatch_ime(
        &mut events,
        &puri::handler::ImeEvent::Commit("entered".into())
    ));
    let ime_fields = events[8].2.as_record().unwrap();
    assert_eq!(
        ime_fields.get(&event_fields::EVENT_KIND),
        Some(&Value::Cell(event_fields::IME))
    );
    assert_eq!(
        ime_fields.get(&event_fields::CONTENT),
        Some(&text::value("entered"))
    );

    let outside = PointerState {
        position: (-100.0, -100.0).into(),
        ..Default::default()
    };
    let button = PointerButtonEvent {
        pointer: touch,
        button: None,
        state: outside.clone(),
    };
    assert!(!handler.dispatch_pointer_down(&mut events, &button));
    assert!(
        !handler
            .dispatch_scroll(
                &mut events,
                &PointerScrollEvent {
                    pointer: touch,
                    state: outside.clone(),
                    delta: ScrollDelta::LineDelta(0.0, 1.0),
                }
            )
            .handled()
    );
    assert_eq!(events.len(), 9);
    assert!(handler.dispatch_pointer_move(
        &mut events,
        &PointerUpdate {
            pointer: touch,
            current: outside,
            coalesced: vec![],
            predicted: vec![],
        }
    ));
    assert!(handler.dispatch_pointer_up(&mut events, &button));
    assert_eq!(events.len(), 11);
}
